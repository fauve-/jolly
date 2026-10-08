use game_engine::{CenterOwner, GameState, Location, Phase, Power};
use press_engine::{
    Commitment, CommitmentOrigin, PressMessage, PressView, Proposal, SpeechAct, Visibility,
};

use crate::{
    DecisionSnapshot, GenerationMode, ProtocolError, ProtocolResult, Token, TokenId,
    response::encode_proposition,
    token::{location_token, phase_token, power_token, visibility_token, year_token},
};

pub const DEFAULT_CONTEXT_CAPACITY: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextPolicy {
    total_capacity: usize,
    response_reserve: usize,
    include_history: bool,
}

impl ContextPolicy {
    pub const fn new(total_capacity: usize, response_reserve: usize) -> Self {
        Self {
            total_capacity,
            response_reserve,
            include_history: true,
        }
    }

    pub const fn with_history(mut self, include_history: bool) -> Self {
        self.include_history = include_history;
        self
    }

    pub const fn total_capacity(self) -> usize {
        self.total_capacity
    }

    pub const fn response_reserve(self) -> usize {
        self.response_reserve
    }

    pub const fn includes_history(self) -> bool {
        self.include_history
    }
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self::new(DEFAULT_CONTEXT_CAPACITY, 0)
    }
}

pub fn encode_observation(snapshot: &DecisionSnapshot) -> ProtocolResult<Vec<TokenId>> {
    encode_observation_with_policy(snapshot, &ContextPolicy::default())
}

pub fn encode_observation_with_policy(
    snapshot: &DecisionSnapshot,
    policy: &ContextPolicy,
) -> ProtocolResult<Vec<TokenId>> {
    let mut tokens = encode_game_state(snapshot)?;

    if snapshot.game_state().phase() != Phase::Winter
        && let Some(view) = snapshot.diplomacy()
    {
        append_diplomacy(snapshot, view, policy, &mut tokens)?;
    }

    tokens.push(generation_marker(snapshot.mode()));
    ensure_capacity(tokens.len(), policy)?;
    Ok(tokens.into_iter().map(Token::id).collect())
}

fn encode_game_state(snapshot: &DecisionSnapshot) -> ProtocolResult<Vec<Token>> {
    let state = snapshot.game_state();
    let mut tokens = Vec::with_capacity(94);
    tokens.extend([
        Token::Bos,
        power_token(snapshot.acting_power()),
        phase_token(state.phase()),
        year_token(state.year())?,
        Token::Board,
    ]);

    for location in Location::ALL {
        tokens.push(location_token(location));
        tokens.push(
            state
                .occupant(location)
                .map(power_token)
                .unwrap_or(Token::Empty),
        );
    }

    tokens.push(Token::Centers);
    for location in GameState::SUPPLY_CENTERS {
        tokens.push(location_token(location));
        let owner = state.center_owner(location).ok_or_else(|| {
            ProtocolError::IllegalResponse(format!("missing supply-center owner for {location:?}"))
        })?;
        tokens.push(match owner {
            CenterOwner::Power(power) => power_token(power),
            CenterOwner::Neutral => Token::Neutral,
        });
    }
    debug_assert_eq!(tokens.len(), 94);
    Ok(tokens)
}

fn append_diplomacy(
    snapshot: &DecisionSnapshot,
    view: &PressView,
    policy: &ContextPolicy,
    tokens: &mut Vec<Token>,
) -> ProtocolResult<()> {
    let observer = snapshot.acting_power();
    let phase = snapshot.game_state().phase();
    let year = snapshot.game_state().year();

    let inbox: Vec<_> = current_inbox(snapshot, view)
        .into_iter()
        .filter(|message| message_visible_to(message, observer))
        .collect();
    let inbox_records: Vec<_> = inbox
        .iter()
        .map(|message| encode_message_record(message, phase, year))
        .collect::<ProtocolResult<_>>()?;

    let mut proposals: Vec<_> = view
        .outstanding_proposals
        .iter()
        .filter(|proposal| proposal_visible_to(proposal, observer))
        .collect();
    proposals.sort_by_key(|proposal| {
        (
            power_token(proposal.sender).id().value(),
            power_token(proposal.recipient).id().value(),
        )
    });
    let proposal_records: Vec<_> = proposals
        .into_iter()
        .map(encode_proposal_record)
        .collect::<ProtocolResult<_>>()?;

    let mut commitment_records: Vec<_> = view
        .active_commitments
        .iter()
        .filter(|commitment| commitment_visible_to(commitment, observer))
        .map(encode_commitment_record)
        .collect::<ProtocolResult<_>>()?;
    commitment_records.sort();

    let history_records = eligible_history_records(snapshot, view, &inbox)?;

    tokens.push(Token::Diplomacy);
    tokens.push(Token::Inbox);
    append_records(tokens, &inbox_records);
    tokens.push(Token::Proposals);
    append_records(tokens, &proposal_records);
    tokens.push(Token::Commitments);
    append_records(tokens, &commitment_records);
    tokens.push(Token::History);

    // The generation marker is appended by the caller and counts against the
    // same total capacity as this required state and the reserved response.
    let required_prompt = tokens.len() + 1;
    ensure_capacity(required_prompt, policy)?;
    let available_history = policy
        .total_capacity
        .saturating_sub(policy.response_reserve)
        .saturating_sub(required_prompt);
    if policy.include_history {
        append_history_suffix(tokens, &history_records, available_history);
    }
    Ok(())
}

fn current_inbox<'a>(snapshot: &'a DecisionSnapshot, view: &'a PressView) -> Vec<&'a PressMessage> {
    if let Some(decision) = snapshot.press_decision() {
        decision.activation().unread.iter().collect()
    } else {
        view.unread.iter().collect()
    }
}

fn eligible_history_records(
    snapshot: &DecisionSnapshot,
    view: &PressView,
    inbox: &[&PressMessage],
) -> ProtocolResult<Vec<Vec<Token>>> {
    let observer = snapshot.acting_power();
    let mut excluded = vec![false; view.messages.len()];
    let mut before = view.messages.len();
    for unread in inbox.iter().rev() {
        if let Some(index) = (0..before)
            .rev()
            .find(|index| &view.messages[*index] == *unread)
        {
            excluded[index] = true;
            before = index;
        }
    }

    let mut records = Vec::new();
    for (index, message) in view.messages.iter().enumerate() {
        if !excluded[index] && message_visible_to(message, observer) {
            records.push(encode_message_record(
                message,
                snapshot.game_state().phase(),
                snapshot.game_state().year(),
            )?);
        }
    }
    Ok(records)
}

fn append_history_suffix(tokens: &mut Vec<Token>, records: &[Vec<Token>], capacity: usize) {
    let mut selected = Vec::new();
    let mut remaining = capacity;
    for record in records.iter().rev() {
        if record.len() > remaining {
            break;
        }
        remaining -= record.len();
        selected.push(record);
    }
    for record in selected.into_iter().rev() {
        tokens.extend(record.iter().copied());
    }
}

fn append_records(tokens: &mut Vec<Token>, records: &[Vec<Token>]) {
    for record in records {
        tokens.extend(record.iter().copied());
    }
}

fn encode_message_record(
    message: &PressMessage,
    phase: Phase,
    year: u8,
) -> ProtocolResult<Vec<Token>> {
    let mut record = record_header(
        message.sender,
        phase,
        year,
        message.visibility,
        message.recipient,
    )?;
    match &message.act {
        SpeechAct::Accept => record.push(Token::Accept),
        SpeechAct::Reject => record.push(Token::Reject),
        SpeechAct::Propose(terms) => {
            record.push(Token::Propose);
            append_propositions(&mut record, terms);
        }
        SpeechAct::Promise(terms) => {
            record.push(Token::Promise);
            append_propositions(&mut record, terms);
        }
        SpeechAct::Request(terms) => {
            record.push(Token::Request);
            append_propositions(&mut record, terms);
        }
    }
    record.push(Token::End);
    Ok(record)
}

fn encode_proposal_record(proposal: &Proposal) -> ProtocolResult<Vec<Token>> {
    let mut record = record_header(
        proposal.sender,
        proposal.phase,
        proposal.year,
        proposal.visibility,
        proposal.recipient,
    )?;
    record.push(Token::Propose);
    append_propositions(&mut record, &proposal.terms);
    record.push(Token::End);
    Ok(record)
}

fn encode_commitment_record(commitment: &Commitment) -> ProtocolResult<Vec<Token>> {
    let mut record = record_header(
        commitment.actor,
        commitment.phase,
        commitment.year,
        commitment.visibility,
        commitment.counterparty,
    )?;
    record.push(match commitment.origin {
        CommitmentOrigin::Promise => Token::Promise,
        CommitmentOrigin::AcceptedProposal { .. } => Token::Accept,
    });
    record.extend(encode_proposition(commitment.proposition));
    record.push(Token::End);
    Ok(record)
}

fn record_header(
    identity: Power,
    phase: Phase,
    year: u8,
    visibility: Visibility,
    counterparty: Power,
) -> ProtocolResult<Vec<Token>> {
    Ok(vec![
        Token::Bos,
        power_token(identity),
        phase_token(phase),
        year_token(year)?,
        visibility_token(visibility),
        power_token(counterparty),
    ])
}

fn append_propositions(tokens: &mut Vec<Token>, terms: &[press_engine::Proposition]) {
    for &term in terms {
        tokens.extend(encode_proposition(term));
    }
}

fn message_visible_to(message: &PressMessage, observer: Power) -> bool {
    message.visibility == Visibility::Public
        || message.sender == observer
        || message.recipient == observer
}

fn proposal_visible_to(proposal: &Proposal, observer: Power) -> bool {
    proposal.visibility == Visibility::Public
        || proposal.sender == observer
        || proposal.recipient == observer
}

fn commitment_visible_to(commitment: &Commitment, observer: Power) -> bool {
    commitment.visibility == Visibility::Public
        || commitment.actor == observer
        || commitment.counterparty == observer
}

fn ensure_capacity(prompt_tokens: usize, policy: &ContextPolicy) -> ProtocolResult<()> {
    let required = prompt_tokens.saturating_add(policy.response_reserve);
    if required > policy.total_capacity {
        return Err(ProtocolError::ContextOverflow {
            required,
            capacity: policy.total_capacity,
        });
    }
    Ok(())
}

const fn generation_marker(mode: GenerationMode) -> Token {
    match mode {
        GenerationMode::Movement => Token::Orders,
        GenerationMode::Adjustment => Token::Adjust,
        GenerationMode::Press => Token::Press,
    }
}

use std::collections::{BTreeMap, HashSet};

use game_engine::{GameState, Location, Order, Phase, Power, is_legal_order};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Proposition {
    Order { actor: Power, order: Order },
    Avoid { actor: Power, location: Location },
}

impl Proposition {
    pub const fn actor(self) -> Power {
        match self {
            Self::Order { actor, .. } | Self::Avoid { actor, .. } => actor,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpeechAct {
    Request(Vec<Proposition>),
    Promise(Vec<Proposition>),
    Propose(Vec<Proposition>),
    Accept,
    Reject,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PressMessage {
    pub sender: Power,
    pub recipient: Power,
    pub visibility: Visibility,
    pub act: SpeechAct,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PressConfig {
    pub max_press_messages_per_power: usize,
}

impl Default for PressConfig {
    fn default() -> Self {
        Self {
            max_press_messages_per_power: 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PressError {
    #[error("press is available only during Spring and Autumn movement phases")]
    NotMovementPhase,
    #[error("the press phase is closed")]
    PhaseClosed,
    #[error("another press activation is already in progress")]
    ActivationInProgress,
    #[error("there is no press activation awaiting a response")]
    NoActivation,
    #[error("the power has neither an opening opportunity nor unread messages")]
    PowerNotEligible,
    #[error("the message sender does not match the activated power")]
    WrongSender,
    #[error("a press message cannot be addressed to its sender")]
    SelfRecipient,
    #[error("the sender's press-message budget is exhausted")]
    BudgetExhausted,
    #[error("PROMISE, REQUEST, and PROPOSE require at least one term")]
    EmptyBundle,
    #[error("a proposition names an actor not permitted by the speech act")]
    InvalidActor,
    #[error("a proposition contains an illegal order or an order for another power's unit")]
    InvalidOrder,
    #[error("a power cannot promise to avoid a location it currently occupies")]
    InvalidAvoidance,
    #[error("the message contains mechanically contradictory terms")]
    ContradictoryTerms,
    #[error("the message contains an exact duplicate term")]
    DuplicateTerm,
    #[error("there is no matching outstanding proposal")]
    NoOutstandingProposal,
    #[error("press cannot close until all openings and unread inboxes are consumed")]
    PhaseNotComplete,
    #[error("commitments cannot be resolved before the press phase closes")]
    MovementNotReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitmentOrigin {
    Promise,
    AcceptedProposal {
        proposal_visibility: Visibility,
        acceptance_visibility: Visibility,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commitment {
    pub actor: Power,
    pub proposition: Proposition,
    pub phase: Phase,
    pub year: u8,
    pub origin: CommitmentOrigin,
    pub counterparty: Power,
    /// Visibility of the terms, not merely of a later acceptance.
    pub visibility: Visibility,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub sender: Power,
    pub recipient: Power,
    pub terms: Vec<Proposition>,
    pub phase: Phase,
    pub year: u8,
    pub visibility: Visibility,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitmentStatus {
    Fulfilled,
    Violated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentResult {
    pub commitment: Commitment,
    pub status: CommitmentStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PressActivation {
    pub power: Power,
    pub is_opening: bool,
    pub unread: Vec<PressMessage>,
    pub can_send: bool,
}

/// The complete diplomatic state visible to one power. Hidden private state is
/// omitted rather than represented by a placeholder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PressView {
    pub messages: Vec<PressMessage>,
    pub unread: Vec<PressMessage>,
    pub outstanding_proposals: Vec<Proposal>,
    pub active_commitments: Vec<Commitment>,
    pub commitment_results: Vec<CommitmentResult>,
    pub sent_count: usize,
    pub opening_complete: bool,
    pub can_send: bool,
    pub closed: bool,
}

/// Typed, deterministic negotiation state for one upcoming movement phase.
///
/// Activating a power consumes the unread snapshot immediately. A rejected
/// response consumes that activation but changes no semantic negotiation state
/// and spends no message budget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PressPhase {
    game_state: GameState,
    config: PressConfig,
    messages: Vec<PressMessage>,
    inboxes: BTreeMap<Power, Vec<PressMessage>>,
    outstanding_proposals: BTreeMap<(Power, Power), Proposal>,
    commitments: Vec<Commitment>,
    commitment_results: Vec<CommitmentResult>,
    sent_count: BTreeMap<Power, usize>,
    openings_complete: HashSet<Power>,
    active_power: Option<Power>,
    closed: bool,
}

impl PressPhase {
    pub fn new(game_state: &GameState) -> Result<Self, PressError> {
        Self::with_config(game_state, PressConfig::default())
    }

    pub fn with_config(game_state: &GameState, config: PressConfig) -> Result<Self, PressError> {
        if !matches!(game_state.phase(), Phase::Spring | Phase::Autumn) {
            return Err(PressError::NotMovementPhase);
        }

        Ok(Self {
            game_state: game_state.clone(),
            config,
            messages: Vec::new(),
            inboxes: Power::ALL
                .into_iter()
                .map(|power| (power, Vec::new()))
                .collect(),
            outstanding_proposals: BTreeMap::new(),
            commitments: Vec::new(),
            commitment_results: Vec::new(),
            sent_count: Power::ALL.into_iter().map(|power| (power, 0)).collect(),
            openings_complete: HashSet::new(),
            active_power: None,
            closed: false,
        })
    }

    pub fn activate(&mut self, power: Power) -> Result<PressActivation, PressError> {
        if self.closed {
            return Err(PressError::PhaseClosed);
        }
        if self.active_power.is_some() {
            return Err(PressError::ActivationInProgress);
        }

        let is_opening = !self.openings_complete.contains(&power);
        if !is_opening && self.inboxes[&power].is_empty() {
            return Err(PressError::PowerNotEligible);
        }

        if is_opening {
            self.openings_complete.insert(power);
        }
        let unread = std::mem::take(self.inboxes.get_mut(&power).unwrap());
        self.active_power = Some(power);
        Ok(PressActivation {
            power,
            is_opening,
            unread,
            can_send: self.can_send(power),
        })
    }

    /// Activates the next eligible power using a stable policy: unfinished
    /// openings in `Power::ALL` order, then unread inboxes in that order.
    pub fn activate_next(&mut self) -> Result<Option<PressActivation>, PressError> {
        if self.closed {
            return Err(PressError::PhaseClosed);
        }
        if self.active_power.is_some() {
            return Err(PressError::ActivationInProgress);
        }
        let next = Power::ALL
            .into_iter()
            .find(|power| !self.openings_complete.contains(power))
            .or_else(|| {
                Power::ALL
                    .into_iter()
                    .find(|power| !self.inboxes[power].is_empty())
            });
        next.map(|power| self.activate(power)).transpose()
    }

    /// Completes the current activation. `None` is the semantic `END` output.
    pub fn respond(&mut self, response: Option<PressMessage>) -> Result<(), PressError> {
        let Some(active_power) = self.active_power.take() else {
            return Err(PressError::NoActivation);
        };
        let Some(message) = response else {
            return Ok(());
        };

        // Validate before applying any semantic transition. This is the
        // transaction boundary used for invalid generated output.
        let normalized = validate_press_message(
            &self.game_state,
            self.outstanding_proposals.values(),
            active_power,
            self.can_send(active_power),
            message,
        )?;
        self.apply_message(normalized);
        Ok(())
    }

    pub fn can_send(&self, power: Power) -> bool {
        self.sent_count[&power] < self.config.max_press_messages_per_power
    }

    pub fn sent_count(&self, power: Power) -> usize {
        self.sent_count[&power]
    }

    pub fn unread_count(&self, power: Power) -> usize {
        self.inboxes[&power].len()
    }

    pub fn messages(&self) -> &[PressMessage] {
        &self.messages
    }

    pub fn commitments(&self) -> &[Commitment] {
        &self.commitments
    }

    pub fn commitment_results(&self) -> &[CommitmentResult] {
        &self.commitment_results
    }

    pub fn outstanding_proposal(&self, sender: Power, recipient: Power) -> Option<&Proposal> {
        self.outstanding_proposals.get(&(sender, recipient))
    }

    pub fn outstanding_proposal_count(&self) -> usize {
        self.outstanding_proposals.len()
    }

    pub fn can_close(&self) -> bool {
        !self.closed
            && self.active_power.is_none()
            && self.openings_complete.len() == Power::ALL.len()
            && self.inboxes.values().all(Vec::is_empty)
    }

    pub fn close(&mut self) -> Result<(), PressError> {
        if self.closed {
            return Err(PressError::PhaseClosed);
        }
        if !self.can_close() {
            return Err(PressError::PhaseNotComplete);
        }
        self.outstanding_proposals.clear();
        self.closed = true;
        Ok(())
    }

    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn view_for(&self, power: Power) -> PressView {
        PressView {
            messages: self
                .messages
                .iter()
                .filter(|message| message_visible_to(message, power))
                .cloned()
                .collect(),
            unread: self.inboxes[&power].clone(),
            outstanding_proposals: self
                .outstanding_proposals
                .values()
                .filter(|proposal| proposal_visible_to(proposal, power))
                .cloned()
                .collect(),
            active_commitments: self
                .commitments
                .iter()
                .filter(|commitment| commitment_visible_to(commitment, power))
                .cloned()
                .collect(),
            commitment_results: self
                .commitment_results
                .iter()
                .filter(|result| commitment_visible_to(&result.commitment, power))
                .cloned()
                .collect(),
            sent_count: self.sent_count(power),
            opening_complete: self.openings_complete.contains(&power),
            can_send: !self.closed && self.can_send(power),
            closed: self.closed,
        }
    }

    /// Classifies all active commitments against raw submitted orders. This is
    /// deliberately independent of adjudication and its fallback holds.
    pub fn resolve_commitments(
        &mut self,
        submitted_orders: &[Order],
    ) -> Result<&[CommitmentResult], PressError> {
        if !self.closed {
            return Err(PressError::MovementNotReady);
        }
        let resolved: Vec<_> = self
            .commitments
            .drain(..)
            .map(|commitment| CommitmentResult {
                status: evaluate_proposition(
                    commitment.actor,
                    commitment.proposition,
                    &self.game_state,
                    submitted_orders,
                ),
                commitment,
            })
            .collect();
        self.commitment_results.extend(resolved);
        Ok(&self.commitment_results)
    }

    fn apply_message(&mut self, message: PressMessage) {
        match &message.act {
            SpeechAct::Request(_) => {}
            SpeechAct::Promise(terms) => {
                self.commitments
                    .extend(terms.iter().copied().map(|term| Commitment {
                        actor: term.actor(),
                        proposition: term,
                        phase: self.game_state.phase(),
                        year: self.game_state.year(),
                        origin: CommitmentOrigin::Promise,
                        counterparty: message.recipient,
                        visibility: message.visibility,
                    }));
            }
            SpeechAct::Propose(terms) => {
                self.outstanding_proposals.insert(
                    (message.sender, message.recipient),
                    Proposal {
                        sender: message.sender,
                        recipient: message.recipient,
                        terms: terms.clone(),
                        phase: self.game_state.phase(),
                        year: self.game_state.year(),
                        visibility: message.visibility,
                    },
                );
            }
            SpeechAct::Accept => {
                let proposal = self
                    .outstanding_proposals
                    .remove(&(message.recipient, message.sender))
                    .expect("validated acceptance must have a proposal");
                self.commitments
                    .extend(proposal.terms.iter().copied().map(|term| Commitment {
                        actor: term.actor(),
                        proposition: term,
                        phase: proposal.phase,
                        year: proposal.year,
                        origin: CommitmentOrigin::AcceptedProposal {
                            proposal_visibility: proposal.visibility,
                            acceptance_visibility: message.visibility,
                        },
                        counterparty: if term.actor() == proposal.sender {
                            proposal.recipient
                        } else {
                            proposal.sender
                        },
                        // A public acceptance reveals an agreement exists, but
                        // does not publish private proposal terms.
                        visibility: proposal.visibility,
                    }));
            }
            SpeechAct::Reject => {
                self.outstanding_proposals
                    .remove(&(message.recipient, message.sender));
            }
        }

        self.inboxes
            .get_mut(&message.recipient)
            .unwrap()
            .push(message.clone());
        self.messages.push(message.clone());
        *self.sent_count.get_mut(&message.sender).unwrap() += 1;
    }
}

/// Validates and canonically normalizes a response without mutating press
/// state. Protocol parsing and masking use this as the authoritative semantic
/// boundary before the caller submits the response to [`PressPhase::respond`].
pub fn validate_press_message<'a>(
    game_state: &GameState,
    outstanding_proposals: impl IntoIterator<Item = &'a Proposal>,
    expected_sender: Power,
    can_send: bool,
    mut message: PressMessage,
) -> Result<PressMessage, PressError> {
    if message.sender != expected_sender {
        return Err(PressError::WrongSender);
    }
    if !can_send {
        return Err(PressError::BudgetExhausted);
    }
    if message.sender == message.recipient {
        return Err(PressError::SelfRecipient);
    }

    let proposals: Vec<_> = outstanding_proposals.into_iter().collect();
    match &mut message.act {
        SpeechAct::Accept | SpeechAct::Reject => {
            if !proposals.iter().any(|proposal| {
                proposal.sender == message.recipient && proposal.recipient == message.sender
            }) {
                return Err(PressError::NoOutstandingProposal);
            }
        }
        SpeechAct::Request(terms) => {
            validate_terms(game_state, terms, &[message.recipient])?;
        }
        SpeechAct::Promise(terms) => {
            validate_terms(game_state, terms, &[message.sender])?;
        }
        SpeechAct::Propose(terms) => {
            validate_terms(game_state, terms, &[message.sender, message.recipient])?;
        }
    }
    Ok(message)
}

fn validate_terms(
    game_state: &GameState,
    terms: &mut [Proposition],
    allowed_actors: &[Power],
) -> Result<(), PressError> {
    if terms.is_empty() {
        return Err(PressError::EmptyBundle);
    }
    for &term in terms.iter() {
        if !allowed_actors.contains(&term.actor()) {
            return Err(PressError::InvalidActor);
        }
        validate_term(game_state, term)?;
    }
    if terms_contradict(terms) {
        return Err(PressError::ContradictoryTerms);
    }
    canonicalize_propositions(terms)
}

fn validate_term(game_state: &GameState, term: Proposition) -> Result<(), PressError> {
    match term {
        Proposition::Order { actor, order } => {
            if game_state.occupant(order.source()) != Some(actor)
                || !is_legal_order(game_state, order)
            {
                Err(PressError::InvalidOrder)
            } else {
                Ok(())
            }
        }
        Proposition::Avoid { actor, location } => {
            if game_state.occupant(location) == Some(actor) {
                Err(PressError::InvalidAvoidance)
            } else {
                Ok(())
            }
        }
    }
}

/// Evaluates a proposition against raw orders submitted for the pre-movement
/// board. This helper also permits testing resolution-only edge cases that are
/// intentionally invalid as newly-created press propositions.
pub fn evaluate_proposition(
    actor: Power,
    proposition: Proposition,
    pre_movement_state: &GameState,
    submitted_orders: &[Order],
) -> CommitmentStatus {
    debug_assert_eq!(actor, proposition.actor());
    match proposition {
        Proposition::Order { order, .. } => {
            let matching_source: Vec<_> = submitted_orders
                .iter()
                .copied()
                .filter(|submitted| {
                    submitted.source() == order.source()
                        && pre_movement_state.occupant(submitted.source()) == Some(actor)
                })
                .collect();
            if matching_source == [order] {
                CommitmentStatus::Fulfilled
            } else {
                CommitmentStatus::Violated
            }
        }
        Proposition::Avoid { location, .. } => {
            let violates = submitted_orders.iter().copied().any(|order| {
                pre_movement_state.occupant(order.source()) == Some(actor)
                    && matches!(
                        order,
                        Order::Move { to, .. } | Order::SupportMove { to, .. }
                            if to == location
                    )
            });
            if violates {
                CommitmentStatus::Violated
            } else {
                CommitmentStatus::Fulfilled
            }
        }
    }
}

fn message_visible_to(message: &PressMessage, power: Power) -> bool {
    message.visibility == Visibility::Public
        || message.sender == power
        || message.recipient == power
}

fn proposal_visible_to(proposal: &Proposal, power: Power) -> bool {
    proposal.visibility == Visibility::Public
        || proposal.sender == power
        || proposal.recipient == power
}

fn commitment_visible_to(commitment: &Commitment, power: Power) -> bool {
    commitment.visibility == Visibility::Public
        || commitment.actor == power
        || commitment.counterparty == power
}

fn terms_contradict(terms: &[Proposition]) -> bool {
    for (index, left) in terms.iter().copied().enumerate() {
        for right in terms.iter().copied().skip(index + 1) {
            match (left, right) {
                (
                    Proposition::Order {
                        actor: left_actor,
                        order: left_order,
                    },
                    Proposition::Order {
                        actor: right_actor,
                        order: right_order,
                    },
                ) if left_actor == right_actor
                    && left_order.source() == right_order.source()
                    && left_order != right_order =>
                {
                    return true;
                }
                (
                    Proposition::Order { actor, order },
                    Proposition::Avoid {
                        actor: avoid_actor,
                        location,
                    },
                )
                | (
                    Proposition::Avoid {
                        actor: avoid_actor,
                        location,
                    },
                    Proposition::Order { actor, order },
                ) if actor == avoid_actor && order_enters(order, location) => return true,
                _ => {}
            }
        }
    }
    false
}

fn order_enters(order: Order, location: Location) -> bool {
    matches!(order, Order::Move { to, .. } | Order::SupportMove { to, .. } if to == location)
}

/// Applies the protocol's lexicographic token-order rule and rejects exact
/// duplicates rather than silently collapsing them.
pub fn canonicalize_propositions(terms: &mut [Proposition]) -> Result<(), PressError> {
    terms.sort_by_key(|term| proposition_token_key(*term));
    if terms.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(PressError::DuplicateTerm);
    }
    Ok(())
}

fn proposition_token_key(term: Proposition) -> Vec<u8> {
    let actor = match term.actor() {
        Power::Green => 12,
        Power::Yellow => 13,
        Power::Red => 14,
        Power::Blue => 15,
    };
    match term {
        Proposition::Avoid { location, .. } => vec![actor, 45, 46 + location as u8],
        Proposition::Order { order, .. } => {
            let mut key = vec![actor, 46 + order.source() as u8];
            match order {
                Order::Hold { .. } => key.push(31),
                Order::Move { to, .. } => key.extend([32, 46 + to as u8]),
                Order::SupportHold { unit, .. } => {
                    key.extend([33, 46 + unit as u8, 31]);
                }
                Order::SupportMove { unit, to, .. } => {
                    key.extend([33, 46 + unit as u8, 32, 46 + to as u8]);
                }
            }
            key
        }
    }
}

use game_engine::{GameState, Location, Order, Power, is_legal_order};
use press_engine::{PressMessage, Proposition, SpeechAct, Visibility, validate_press_message};

use crate::{
    DecisionSnapshot, GenerationMode, ProtocolError, ProtocolResult, Token, TokenId, TokenMask,
    TypedResponse, parse_response,
    response::encode_proposition,
    token::{location_token, power_token, token_location, token_power, token_visibility},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResponseBudget(usize);

impl ResponseBudget {
    pub const fn new(max_tokens: usize) -> Self {
        Self(max_tokens)
    }

    pub const fn max_tokens(self) -> usize {
        self.0
    }
}

#[derive(Clone, Debug)]
pub struct Decoder {
    snapshot: DecisionSnapshot,
    budget: ResponseBudget,
    prefix: Vec<TokenId>,
    terminal: bool,
}

impl Decoder {
    pub fn new(snapshot: &DecisionSnapshot, budget: ResponseBudget) -> ProtocolResult<Self> {
        let minimum = analyze(snapshot, &[])?.minimum_remaining;
        if budget.max_tokens() < minimum {
            return Err(ProtocolError::InsufficientBudget {
                provided: budget.max_tokens(),
                minimum,
            });
        }
        Ok(Self {
            snapshot: snapshot.clone(),
            budget,
            prefix: Vec::new(),
            terminal: false,
        })
    }

    pub fn prefix(&self) -> &[TokenId] {
        &self.prefix
    }

    pub const fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub fn valid_next_tokens(&self) -> TokenMask {
        if self.terminal {
            return TokenMask::empty();
        }
        let analysis = analyze(&self.snapshot, &self.prefix)
            .expect("a decoder reached only through push must retain a valid prefix");
        let mut mask = TokenMask::empty();
        for token in analysis.candidates {
            let mut extended = self.prefix.clone();
            extended.push(token.id());
            let Ok(next) = analyze(&self.snapshot, &extended) else {
                continue;
            };
            if extended.len() + next.minimum_remaining <= self.budget.max_tokens() {
                mask.insert(token);
            }
        }
        mask
    }

    pub fn push(&mut self, token: TokenId) -> ProtocolResult<()> {
        if self.terminal {
            return Err(ProtocolError::DecoderTerminal);
        }
        let allowed = self.valid_next_tokens();
        if !allowed.contains(token.token()) {
            return Err(ProtocolError::InvalidPrefix {
                position: self.prefix.len(),
                reason: format!("{} is not an allowed continuation", token.token()),
            });
        }
        self.prefix.push(token);
        self.terminal = token.token() == Token::End;
        Ok(())
    }

    pub fn finish(self) -> ProtocolResult<TypedResponse> {
        if !self.terminal {
            return Err(ProtocolError::DecoderIncomplete);
        }
        parse_response(&self.snapshot, &self.prefix)
    }
}

struct Analysis {
    candidates: Vec<Token>,
    minimum_remaining: usize,
}

fn analyze(snapshot: &DecisionSnapshot, prefix: &[TokenId]) -> ProtocolResult<Analysis> {
    match snapshot.mode() {
        GenerationMode::Movement => analyze_movement(snapshot, prefix),
        GenerationMode::Adjustment => analyze_adjustment(snapshot, prefix),
        GenerationMode::Press => analyze_press(snapshot, prefix),
    }
}

fn owned_sources(state: &GameState, power: Power) -> Vec<Location> {
    Location::ALL
        .into_iter()
        .filter(|location| state.occupant(*location) == Some(power))
        .collect()
}

#[derive(Clone, Copy)]
enum MoveStage {
    Source(usize),
    Action {
        index: usize,
        source: Location,
    },
    MoveDestination {
        index: usize,
        source: Location,
    },
    SupportUnit {
        index: usize,
        source: Location,
    },
    SupportAction {
        index: usize,
        source: Location,
        unit: Location,
    },
    SupportDestination {
        index: usize,
        source: Location,
        unit: Location,
    },
    Done,
}

fn analyze_movement(snapshot: &DecisionSnapshot, prefix: &[TokenId]) -> ProtocolResult<Analysis> {
    let state = snapshot.game_state();
    let sources = owned_sources(state, snapshot.acting_power());
    let mut stage = MoveStage::Source(0);
    for (position, id) in prefix.iter().copied().enumerate() {
        let token = id.token();
        let candidates = movement_candidates(state, &sources, stage);
        if !candidates.contains(&token) {
            return invalid(position, token);
        }
        stage = advance_movement(&sources, stage, token)?;
    }
    Ok(Analysis {
        candidates: movement_candidates(state, &sources, stage),
        minimum_remaining: movement_minimum(state, &sources, stage),
    })
}

fn movement_candidates(state: &GameState, sources: &[Location], stage: MoveStage) -> Vec<Token> {
    match stage {
        MoveStage::Source(index) if index == sources.len() => vec![Token::End],
        MoveStage::Source(index) => vec![location_token(sources[index])],
        MoveStage::Action { source, .. } => {
            let mut result = vec![Token::Hold];
            if Location::ALL
                .into_iter()
                .any(|to| is_legal_order(state, Order::Move { from: source, to }))
            {
                result.push(Token::Move);
            }
            if !support_units(state, source).is_empty() {
                result.push(Token::Support);
            }
            result
        }
        MoveStage::MoveDestination { source, .. } => Location::ALL
            .into_iter()
            .filter(|to| {
                is_legal_order(
                    state,
                    Order::Move {
                        from: source,
                        to: *to,
                    },
                )
            })
            .map(location_token)
            .collect(),
        MoveStage::SupportUnit { source, .. } => support_units(state, source)
            .into_iter()
            .map(location_token)
            .collect(),
        MoveStage::SupportAction { source, unit, .. } => {
            let mut result = Vec::new();
            if is_legal_order(state, Order::SupportHold { from: source, unit }) {
                result.push(Token::Hold);
            }
            if Location::ALL.into_iter().any(|to| {
                is_legal_order(
                    state,
                    Order::SupportMove {
                        from: source,
                        unit,
                        to,
                    },
                )
            }) {
                result.push(Token::Move);
            }
            result
        }
        MoveStage::SupportDestination { source, unit, .. } => Location::ALL
            .into_iter()
            .filter(|to| {
                is_legal_order(
                    state,
                    Order::SupportMove {
                        from: source,
                        unit,
                        to: *to,
                    },
                )
            })
            .map(location_token)
            .collect(),
        MoveStage::Done => Vec::new(),
    }
}

fn support_units(state: &GameState, source: Location) -> Vec<Location> {
    Location::ALL
        .into_iter()
        .filter(|unit| {
            is_legal_order(
                state,
                Order::SupportHold {
                    from: source,
                    unit: *unit,
                },
            ) || Location::ALL.into_iter().any(|to| {
                is_legal_order(
                    state,
                    Order::SupportMove {
                        from: source,
                        unit: *unit,
                        to,
                    },
                )
            })
        })
        .collect()
}

fn next_source(sources: &[Location], index: usize) -> MoveStage {
    MoveStage::Source((index + 1).min(sources.len()))
}

fn advance_movement(
    sources: &[Location],
    stage: MoveStage,
    token: Token,
) -> ProtocolResult<MoveStage> {
    Ok(match stage {
        MoveStage::Source(index) if index == sources.len() && token == Token::End => {
            MoveStage::Done
        }
        MoveStage::Source(index) => MoveStage::Action {
            index,
            source: sources[index],
        },
        MoveStage::Action { index, source } => match token {
            Token::Hold => next_source(sources, index),
            Token::Move => MoveStage::MoveDestination { index, source },
            Token::Support => MoveStage::SupportUnit { index, source },
            _ => unreachable!(),
        },
        MoveStage::MoveDestination { index, .. } => next_source(sources, index),
        MoveStage::SupportUnit { index, source } => MoveStage::SupportAction {
            index,
            source,
            unit: token_location(token).expect("candidate is a location"),
        },
        MoveStage::SupportAction {
            index,
            source,
            unit,
        } => match token {
            Token::Hold => next_source(sources, index),
            Token::Move => MoveStage::SupportDestination {
                index,
                source,
                unit,
            },
            _ => unreachable!(),
        },
        MoveStage::SupportDestination { index, .. } => next_source(sources, index),
        MoveStage::Done => return Err(ProtocolError::DecoderTerminal),
    })
}

fn movement_minimum(state: &GameState, sources: &[Location], stage: MoveStage) -> usize {
    let after_current = |index: usize| 2 * (sources.len() - index - 1) + 1;
    match stage {
        MoveStage::Source(index) => 2 * (sources.len() - index) + 1,
        MoveStage::Action { index, .. } => 1 + after_current(index),
        MoveStage::MoveDestination { index, .. } => 1 + after_current(index),
        MoveStage::SupportUnit { index, source, .. } => {
            let shortest = support_units(state, source)
                .into_iter()
                .map(|unit| {
                    if is_legal_order(state, Order::SupportHold { from: source, unit }) {
                        2
                    } else {
                        3
                    }
                })
                .min()
                .unwrap_or(usize::MAX / 2);
            shortest + after_current(index)
        }
        MoveStage::SupportAction {
            index,
            source,
            unit,
        } => {
            let shortest = if is_legal_order(state, Order::SupportHold { from: source, unit }) {
                1
            } else {
                2
            };
            shortest + after_current(index)
        }
        MoveStage::SupportDestination { index, .. } => 1 + after_current(index),
        MoveStage::Done => 0,
    }
}

#[derive(Clone, Copy)]
enum AdjustmentStage {
    Start,
    BuildLocation,
    DisbandKeyword {
        index: usize,
        last: Option<Location>,
    },
    DisbandLocation {
        index: usize,
        last: Option<Location>,
    },
    End,
    Done,
}

fn adjustment_delta(snapshot: &DecisionSnapshot) -> isize {
    let state = snapshot.game_state();
    state.center_count(snapshot.acting_power()) as isize
        - state.army_count(snapshot.acting_power()) as isize
}

fn analyze_adjustment(snapshot: &DecisionSnapshot, prefix: &[TokenId]) -> ProtocolResult<Analysis> {
    let delta = adjustment_delta(snapshot);
    let mut stage = if delta < 0 {
        AdjustmentStage::DisbandKeyword {
            index: 0,
            last: None,
        }
    } else {
        AdjustmentStage::Start
    };
    for (position, id) in prefix.iter().copied().enumerate() {
        let token = id.token();
        let candidates = adjustment_candidates(snapshot, delta, stage);
        if !candidates.contains(&token) {
            return invalid(position, token);
        }
        stage = advance_adjustment(delta, stage, token);
    }
    Ok(Analysis {
        candidates: adjustment_candidates(snapshot, delta, stage),
        minimum_remaining: adjustment_minimum(delta, stage),
    })
}

fn adjustment_candidates(
    snapshot: &DecisionSnapshot,
    delta: isize,
    stage: AdjustmentStage,
) -> Vec<Token> {
    match stage {
        AdjustmentStage::Start if delta == 0 => vec![Token::None],
        AdjustmentStage::Start => {
            let state = snapshot.game_state();
            let power = snapshot.acting_power();
            let mut result = vec![Token::Waive];
            let home = power.home_center();
            if state.center_owner(home) == Some(game_engine::CenterOwner::Power(power))
                && state.occupant(home).is_none()
            {
                result.insert(0, Token::Build);
            }
            result
        }
        AdjustmentStage::BuildLocation => {
            vec![location_token(snapshot.acting_power().home_center())]
        }
        AdjustmentStage::DisbandKeyword { .. } => vec![Token::Disband],
        AdjustmentStage::DisbandLocation { index, last } => {
            let required = delta.unsigned_abs();
            let remaining_after = required - index - 1;
            let armies = owned_sources(snapshot.game_state(), snapshot.acting_power());
            armies
                .iter()
                .copied()
                .filter(|location| last.is_none_or(|previous| *location > previous))
                .filter(|location| {
                    armies.iter().filter(|other| **other > *location).count() >= remaining_after
                })
                .map(location_token)
                .collect()
        }
        AdjustmentStage::End => vec![Token::End],
        AdjustmentStage::Done => Vec::new(),
    }
}

fn advance_adjustment(delta: isize, stage: AdjustmentStage, token: Token) -> AdjustmentStage {
    match stage {
        AdjustmentStage::Start => match token {
            Token::Build => AdjustmentStage::BuildLocation,
            Token::Waive | Token::None => AdjustmentStage::End,
            _ => unreachable!(),
        },
        AdjustmentStage::BuildLocation => AdjustmentStage::End,
        AdjustmentStage::DisbandKeyword { index, last } => {
            AdjustmentStage::DisbandLocation { index, last }
        }
        AdjustmentStage::DisbandLocation { index, .. } => {
            let next = index + 1;
            if next == delta.unsigned_abs() {
                AdjustmentStage::End
            } else {
                AdjustmentStage::DisbandKeyword {
                    index: next,
                    last: token_location(token),
                }
            }
        }
        AdjustmentStage::End => AdjustmentStage::Done,
        AdjustmentStage::Done => AdjustmentStage::Done,
    }
}

fn adjustment_minimum(delta: isize, stage: AdjustmentStage) -> usize {
    match stage {
        AdjustmentStage::Start => 2,
        AdjustmentStage::BuildLocation => 2,
        AdjustmentStage::DisbandKeyword { index, .. } => 2 * (delta.unsigned_abs() - index) + 1,
        AdjustmentStage::DisbandLocation { index, .. } => {
            1 + 2 * (delta.unsigned_abs() - index - 1) + 1
        }
        AdjustmentStage::End => 1,
        AdjustmentStage::Done => 0,
    }
}

#[derive(Clone, Copy)]
enum PressActKind {
    Promise,
    Request,
    Propose,
}

#[derive(Clone)]
enum PressStage {
    Start,
    Recipient {
        visibility: Visibility,
    },
    Act {
        visibility: Visibility,
        recipient: Power,
    },
    End,
    Terms {
        visibility: Visibility,
        recipient: Power,
        kind: PressActKind,
        completed: Vec<Proposition>,
        partial: Vec<Token>,
    },
    Done,
}

fn analyze_press(snapshot: &DecisionSnapshot, prefix: &[TokenId]) -> ProtocolResult<Analysis> {
    let mut stage = PressStage::Start;
    for (position, id) in prefix.iter().copied().enumerate() {
        let token = id.token();
        let candidates = press_candidates(snapshot, &stage)?;
        if !candidates.contains(&token) {
            return invalid(position, token);
        }
        stage = advance_press(snapshot, stage, token)?;
    }
    Ok(Analysis {
        candidates: press_candidates(snapshot, &stage)?,
        minimum_remaining: press_minimum(snapshot, &stage)?,
    })
}

fn press_candidates(snapshot: &DecisionSnapshot, stage: &PressStage) -> ProtocolResult<Vec<Token>> {
    let decision = snapshot
        .press_decision()
        .ok_or(ProtocolError::MissingPressDecision)?;
    Ok(match stage {
        PressStage::Start => {
            let mut result = vec![Token::End];
            if decision.activation().can_send {
                result.extend([Token::Public, Token::Private]);
            }
            result
        }
        PressStage::Recipient { .. } => Power::ALL
            .into_iter()
            .filter(|power| *power != snapshot.acting_power())
            .map(power_token)
            .collect(),
        PressStage::Act {
            visibility,
            recipient,
        } => {
            let mut result = Vec::new();
            if has_incoming_proposal(snapshot, *recipient) {
                result.extend([Token::Accept, Token::Reject]);
            }
            for (token, kind) in [
                (Token::Propose, PressActKind::Propose),
                (Token::Promise, PressActKind::Promise),
                (Token::Request, PressActKind::Request),
            ] {
                if !valid_next_terms(snapshot, *visibility, *recipient, kind, &[]).is_empty() {
                    result.push(token);
                }
            }
            result
        }
        PressStage::End => vec![Token::End],
        PressStage::Terms {
            visibility,
            recipient,
            kind,
            completed,
            partial,
        } => {
            let terms = valid_next_terms(snapshot, *visibility, *recipient, *kind, completed);
            let mut result = Vec::new();
            if partial.is_empty() && !completed.is_empty() {
                result.push(Token::End);
            }
            for (tokens, _) in terms {
                if tokens.starts_with(partial) && tokens.len() > partial.len() {
                    let next = tokens[partial.len()];
                    if !result.contains(&next) {
                        result.push(next);
                    }
                }
            }
            result.sort();
            result
        }
        PressStage::Done => Vec::new(),
    })
}

fn advance_press(
    snapshot: &DecisionSnapshot,
    stage: PressStage,
    token: Token,
) -> ProtocolResult<PressStage> {
    Ok(match stage {
        PressStage::Start if token == Token::End => PressStage::Done,
        PressStage::Start => PressStage::Recipient {
            visibility: token_visibility(token).expect("candidate is visibility"),
        },
        PressStage::Recipient { visibility } => PressStage::Act {
            visibility,
            recipient: token_power(token).expect("candidate is power"),
        },
        PressStage::Act {
            visibility,
            recipient,
        } => match token {
            Token::Accept | Token::Reject => PressStage::End,
            Token::Promise | Token::Request | Token::Propose => PressStage::Terms {
                visibility,
                recipient,
                kind: match token {
                    Token::Promise => PressActKind::Promise,
                    Token::Request => PressActKind::Request,
                    Token::Propose => PressActKind::Propose,
                    _ => unreachable!(),
                },
                completed: Vec::new(),
                partial: Vec::new(),
            },
            _ => unreachable!(),
        },
        PressStage::End => PressStage::Done,
        PressStage::Terms {
            visibility,
            recipient,
            kind,
            mut completed,
            mut partial,
        } => {
            if token == Token::End && partial.is_empty() && !completed.is_empty() {
                PressStage::Done
            } else {
                partial.push(token);
                let terms = valid_next_terms(snapshot, visibility, recipient, kind, &completed);
                if let Some((_, term)) = terms
                    .iter()
                    .find(|(tokens, _)| tokens.as_slice() == partial.as_slice())
                {
                    completed.push(*term);
                    partial.clear();
                }
                PressStage::Terms {
                    visibility,
                    recipient,
                    kind,
                    completed,
                    partial,
                }
            }
        }
        PressStage::Done => return Err(ProtocolError::DecoderTerminal),
    })
}

fn press_minimum(snapshot: &DecisionSnapshot, stage: &PressStage) -> ProtocolResult<usize> {
    Ok(match stage {
        PressStage::Start => 1,
        PressStage::Recipient { visibility } => {
            1 + Power::ALL
                .into_iter()
                .filter(|power| *power != snapshot.acting_power())
                .filter_map(|recipient| minimum_after_recipient(snapshot, *visibility, recipient))
                .min()
                .ok_or_else(|| ProtocolError::IllegalResponse("no legal press recipient".into()))?
        }
        PressStage::Act {
            visibility,
            recipient,
        } => minimum_after_recipient(snapshot, *visibility, *recipient)
            .ok_or_else(|| ProtocolError::IllegalResponse("no legal speech act".into()))?,
        PressStage::End => 1,
        PressStage::Terms {
            visibility,
            recipient,
            kind,
            completed,
            partial,
        } => {
            if partial.is_empty() && !completed.is_empty() {
                1
            } else {
                valid_next_terms(snapshot, *visibility, *recipient, *kind, completed)
                    .into_iter()
                    .filter(|(tokens, _)| tokens.starts_with(partial))
                    .map(|(tokens, _)| tokens.len() - partial.len() + 1)
                    .min()
                    .ok_or_else(|| {
                        ProtocolError::IllegalResponse("press term has no completion".into())
                    })?
            }
        }
        PressStage::Done => 0,
    })
}

fn minimum_after_recipient(
    snapshot: &DecisionSnapshot,
    visibility: Visibility,
    recipient: Power,
) -> Option<usize> {
    let mut lengths = Vec::new();
    if has_incoming_proposal(snapshot, recipient) {
        lengths.push(2); // ACCEPT/REJECT END
    }
    for kind in [
        PressActKind::Promise,
        PressActKind::Request,
        PressActKind::Propose,
    ] {
        if let Some(shortest) = valid_next_terms(snapshot, visibility, recipient, kind, &[])
            .into_iter()
            .map(|(tokens, _)| tokens.len())
            .min()
        {
            lengths.push(1 + shortest + 1); // act, term, END
        }
    }
    lengths.into_iter().min()
}

fn has_incoming_proposal(snapshot: &DecisionSnapshot, recipient: Power) -> bool {
    snapshot.press_decision().is_some_and(|decision| {
        decision
            .view()
            .outstanding_proposals
            .iter()
            .any(|proposal| {
                proposal.sender == recipient && proposal.recipient == snapshot.acting_power()
            })
    })
}

fn valid_next_terms(
    snapshot: &DecisionSnapshot,
    visibility: Visibility,
    recipient: Power,
    kind: PressActKind,
    completed: &[Proposition],
) -> Vec<(Vec<Token>, Proposition)> {
    let actors: Vec<_> = match kind {
        PressActKind::Promise => vec![snapshot.acting_power()],
        PressActKind::Request => vec![recipient],
        PressActKind::Propose => vec![snapshot.acting_power(), recipient],
    };
    let mut result = Vec::new();
    for term in atomic_terms(snapshot.game_state(), &actors) {
        let mut terms = completed.to_vec();
        terms.push(term);
        let act = match kind {
            PressActKind::Promise => SpeechAct::Promise(terms.clone()),
            PressActKind::Request => SpeechAct::Request(terms.clone()),
            PressActKind::Propose => SpeechAct::Propose(terms.clone()),
        };
        let message = PressMessage {
            sender: snapshot.acting_power(),
            recipient,
            visibility,
            act,
        };
        let Some(decision) = snapshot.press_decision() else {
            continue;
        };
        let Ok(normalized) = validate_press_message(
            snapshot.game_state(),
            decision.view().outstanding_proposals.iter(),
            snapshot.acting_power(),
            decision.activation().can_send,
            message.clone(),
        ) else {
            continue;
        };
        if normalized == message {
            result.push((encode_proposition(term), term));
        }
    }
    result.sort_by(|left, right| left.0.cmp(&right.0));
    result
}

fn atomic_terms(state: &GameState, actors: &[Power]) -> Vec<Proposition> {
    let mut terms = Vec::new();
    for &actor in actors {
        for location in Location::ALL {
            if state.occupant(location) != Some(actor) {
                terms.push(Proposition::Avoid { actor, location });
            }
        }
        for source in owned_sources(state, actor) {
            let orders = std::iter::once(Order::Hold { at: source })
                .chain(
                    Location::ALL
                        .into_iter()
                        .map(|to| Order::Move { from: source, to }),
                )
                .chain(
                    Location::ALL
                        .into_iter()
                        .map(|unit| Order::SupportHold { from: source, unit }),
                )
                .chain(Location::ALL.into_iter().flat_map(|unit| {
                    Location::ALL.into_iter().map(move |to| Order::SupportMove {
                        from: source,
                        unit,
                        to,
                    })
                }));
            terms.extend(
                orders
                    .filter(|order| is_legal_order(state, *order))
                    .map(|order| Proposition::Order { actor, order }),
            );
        }
    }
    terms.sort_by_key(|term| encode_proposition(*term));
    terms
}

fn invalid<T>(position: usize, token: Token) -> ProtocolResult<T> {
    Err(ProtocolError::InvalidPrefix {
        position,
        reason: format!("{token} is not legal in this decoder state"),
    })
}

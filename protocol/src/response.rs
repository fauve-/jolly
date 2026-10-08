use std::collections::BTreeMap;

use game_engine::{Adjustment, CenterOwner, GameState, Location, Order, Power, is_legal_order};
use press_engine::{PressMessage, Proposition, SpeechAct, validate_press_message};

use crate::{
    DecisionSnapshot, GenerationMode, ProtocolError, ProtocolResult, Token, TokenId,
    token::{
        location_token, power_token, token_location, token_power, token_visibility,
        visibility_token,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdjustmentResponse {
    None,
    Build(Location),
    Waive,
    Disbands(Vec<Location>),
}

impl AdjustmentResponse {
    pub fn adjustments(&self, power: Power) -> Vec<Adjustment> {
        match self {
            Self::None => Vec::new(),
            Self::Build(at) => vec![Adjustment::Build { at: *at, power }],
            Self::Waive => vec![Adjustment::Waive { power }],
            Self::Disbands(locations) => locations
                .iter()
                .copied()
                .map(|at| Adjustment::Disband { at, power })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypedResponse {
    Movement(Vec<Order>),
    Adjustment(AdjustmentResponse),
    Press(Option<PressMessage>),
}

pub fn encode_response(
    snapshot: &DecisionSnapshot,
    response: &TypedResponse,
) -> ProtocolResult<Vec<TokenId>> {
    let tokens = match (snapshot.mode(), response) {
        (GenerationMode::Movement, TypedResponse::Movement(orders)) => {
            encode_movement(snapshot, orders)?
        }
        (GenerationMode::Adjustment, TypedResponse::Adjustment(adjustment)) => {
            encode_adjustment(snapshot, adjustment)?
        }
        (GenerationMode::Press, TypedResponse::Press(message)) => {
            encode_press(snapshot, message.as_ref())?
        }
        (mode, _) => return Err(ProtocolError::WrongResponseMode { mode }),
    };
    Ok(tokens.into_iter().map(Token::id).collect())
}

pub fn parse_response(
    snapshot: &DecisionSnapshot,
    suffix: &[TokenId],
) -> ProtocolResult<TypedResponse> {
    match snapshot.mode() {
        GenerationMode::Movement => parse_movement(snapshot, suffix).map(TypedResponse::Movement),
        GenerationMode::Adjustment => {
            parse_adjustment(snapshot, suffix).map(TypedResponse::Adjustment)
        }
        GenerationMode::Press => parse_press(snapshot, suffix).map(TypedResponse::Press),
    }
}

fn owned_sources(state: &GameState, power: Power) -> Vec<Location> {
    Location::ALL
        .into_iter()
        .filter(|location| state.occupant(*location) == Some(power))
        .collect()
}

fn canonical_movement(snapshot: &DecisionSnapshot, orders: &[Order]) -> ProtocolResult<Vec<Order>> {
    let state = snapshot.game_state();
    let power = snapshot.acting_power();
    let expected = owned_sources(state, power);
    let mut by_source = BTreeMap::new();
    for &order in orders {
        let source = order.source();
        if by_source.insert(source, order).is_some() {
            return Err(ProtocolError::Duplicate);
        }
        if state.occupant(source) != Some(power) || !is_legal_order(state, order) {
            return Err(ProtocolError::IllegalResponse(format!(
                "illegal order {order:?} for {power:?}"
            )));
        }
    }
    if by_source.keys().copied().collect::<Vec<_>>() != expected {
        return Err(ProtocolError::IncompleteResponse);
    }
    Ok(by_source.into_values().collect())
}

fn encode_movement(snapshot: &DecisionSnapshot, orders: &[Order]) -> ProtocolResult<Vec<Token>> {
    let canonical = canonical_movement(snapshot, orders)?;
    let mut tokens = Vec::new();
    for order in canonical {
        tokens.extend(encode_order(order));
    }
    tokens.push(Token::End);
    Ok(tokens)
}

pub(crate) fn encode_order(order: Order) -> Vec<Token> {
    match order {
        Order::Hold { at } => vec![location_token(at), Token::Hold],
        Order::Move { from, to } => {
            vec![location_token(from), Token::Move, location_token(to)]
        }
        Order::SupportHold { from, unit } => vec![
            location_token(from),
            Token::Support,
            location_token(unit),
            Token::Hold,
        ],
        Order::SupportMove { from, unit, to } => vec![
            location_token(from),
            Token::Support,
            location_token(unit),
            Token::Move,
            location_token(to),
        ],
    }
}

fn parse_movement(snapshot: &DecisionSnapshot, suffix: &[TokenId]) -> ProtocolResult<Vec<Order>> {
    let state = snapshot.game_state();
    let mut reader = Reader::new(suffix);
    let mut orders = Vec::new();
    for source in owned_sources(state, snapshot.acting_power()) {
        reader.expect(location_token(source), "next owned army source")?;
        let order = parse_order_tail(&mut reader, source)?;
        if !is_legal_order(state, order) {
            return Err(ProtocolError::IllegalResponse(format!(
                "illegal order {order:?}"
            )));
        }
        orders.push(order);
    }
    reader.expect_end()?;
    Ok(orders)
}

fn parse_order_tail(reader: &mut Reader<'_>, source: Location) -> ProtocolResult<Order> {
    let action_position = reader.position();
    match reader.next("movement action")? {
        Token::Hold => Ok(Order::Hold { at: source }),
        Token::Move => Ok(Order::Move {
            from: source,
            to: reader.next_location("move destination")?,
        }),
        Token::Support => {
            let unit = reader.next_location("supported unit source")?;
            match reader.next("supported action")? {
                Token::Hold => Ok(Order::SupportHold { from: source, unit }),
                Token::Move => Ok(Order::SupportMove {
                    from: source,
                    unit,
                    to: reader.next_location("supported move destination")?,
                }),
                found => Err(ProtocolError::UnexpectedToken {
                    position: reader.position() - 1,
                    found,
                    expected: "HOLD or MOVE",
                }),
            }
        }
        found => Err(ProtocolError::UnexpectedToken {
            position: action_position,
            found,
            expected: "HOLD, MOVE, or SUPPORT",
        }),
    }
}

fn adjustment_delta(snapshot: &DecisionSnapshot) -> isize {
    let state = snapshot.game_state();
    state.center_count(snapshot.acting_power()) as isize
        - state.army_count(snapshot.acting_power()) as isize
}

fn validate_adjustment(
    snapshot: &DecisionSnapshot,
    response: &AdjustmentResponse,
) -> ProtocolResult<()> {
    let state = snapshot.game_state();
    let power = snapshot.acting_power();
    let delta = adjustment_delta(snapshot);
    match (delta.cmp(&0), response) {
        (std::cmp::Ordering::Equal, AdjustmentResponse::None) => Ok(()),
        (std::cmp::Ordering::Greater, AdjustmentResponse::Waive) => Ok(()),
        (std::cmp::Ordering::Greater, AdjustmentResponse::Build(at))
            if *at == power.home_center()
                && state.center_owner(*at) == Some(CenterOwner::Power(power))
                && state.occupant(*at).is_none() =>
        {
            Ok(())
        }
        (std::cmp::Ordering::Less, AdjustmentResponse::Disbands(locations)) => {
            if locations.len() != delta.unsigned_abs() {
                return Err(ProtocolError::IncompleteResponse);
            }
            if locations.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ProtocolError::NonCanonical { position: 0 });
            }
            if locations
                .iter()
                .any(|location| state.occupant(*location) != Some(power))
            {
                return Err(ProtocolError::IllegalResponse(
                    "disband does not name an acting-power army".into(),
                ));
            }
            Ok(())
        }
        _ => Err(ProtocolError::IllegalResponse(format!(
            "adjustment {response:?} does not match delta {delta}"
        ))),
    }
}

fn encode_adjustment(
    snapshot: &DecisionSnapshot,
    response: &AdjustmentResponse,
) -> ProtocolResult<Vec<Token>> {
    validate_adjustment(snapshot, response)?;
    let mut tokens = match response {
        AdjustmentResponse::None => vec![Token::None],
        AdjustmentResponse::Build(location) => vec![Token::Build, location_token(*location)],
        AdjustmentResponse::Waive => vec![Token::Waive],
        AdjustmentResponse::Disbands(locations) => locations
            .iter()
            .flat_map(|location| [Token::Disband, location_token(*location)])
            .collect(),
    };
    tokens.push(Token::End);
    Ok(tokens)
}

fn parse_adjustment(
    snapshot: &DecisionSnapshot,
    suffix: &[TokenId],
) -> ProtocolResult<AdjustmentResponse> {
    let mut reader = Reader::new(suffix);
    let delta = adjustment_delta(snapshot);
    let response = match delta.cmp(&0) {
        std::cmp::Ordering::Equal => {
            reader.expect(Token::None, "NONE")?;
            AdjustmentResponse::None
        }
        std::cmp::Ordering::Greater => match reader.next("BUILD or WAIVE")? {
            Token::Build => AdjustmentResponse::Build(reader.next_location("build location")?),
            Token::Waive => AdjustmentResponse::Waive,
            found => {
                return Err(ProtocolError::UnexpectedToken {
                    position: reader.position() - 1,
                    found,
                    expected: "BUILD or WAIVE",
                });
            }
        },
        std::cmp::Ordering::Less => {
            let mut locations = Vec::with_capacity(delta.unsigned_abs());
            for _ in 0..delta.unsigned_abs() {
                reader.expect(Token::Disband, "DISBAND")?;
                locations.push(reader.next_location("disband location")?);
            }
            AdjustmentResponse::Disbands(locations)
        }
    };
    reader.expect_end()?;
    validate_adjustment(snapshot, &response)?;
    Ok(response)
}

fn validate_press(
    snapshot: &DecisionSnapshot,
    message: PressMessage,
) -> ProtocolResult<PressMessage> {
    let decision = snapshot
        .press_decision()
        .ok_or(ProtocolError::MissingPressDecision)?;
    validate_press_message(
        snapshot.game_state(),
        decision.view().outstanding_proposals.iter(),
        snapshot.acting_power(),
        decision.activation().can_send,
        message,
    )
    .map_err(|error| ProtocolError::IllegalResponse(error.to_string()))
}

fn encode_press(
    snapshot: &DecisionSnapshot,
    message: Option<&PressMessage>,
) -> ProtocolResult<Vec<Token>> {
    let Some(message) = message else {
        return Ok(vec![Token::End]);
    };
    let normalized = validate_press(snapshot, message.clone())?;
    let mut tokens = vec![
        visibility_token(normalized.visibility),
        power_token(normalized.recipient),
    ];
    match normalized.act {
        SpeechAct::Accept => tokens.push(Token::Accept),
        SpeechAct::Reject => tokens.push(Token::Reject),
        SpeechAct::Request(terms) => {
            tokens.push(Token::Request);
            append_terms(&mut tokens, terms);
        }
        SpeechAct::Promise(terms) => {
            tokens.push(Token::Promise);
            append_terms(&mut tokens, terms);
        }
        SpeechAct::Propose(terms) => {
            tokens.push(Token::Propose);
            append_terms(&mut tokens, terms);
        }
    }
    tokens.push(Token::End);
    Ok(tokens)
}

fn append_terms(tokens: &mut Vec<Token>, terms: Vec<Proposition>) {
    for term in terms {
        tokens.extend(encode_proposition(term));
    }
}

pub(crate) fn encode_proposition(term: Proposition) -> Vec<Token> {
    let mut tokens = vec![power_token(term.actor())];
    match term {
        Proposition::Avoid { location, .. } => {
            tokens.extend([Token::Avoid, location_token(location)]);
        }
        Proposition::Order { order, .. } => tokens.extend(encode_order(order)),
    }
    tokens
}

fn parse_press(
    snapshot: &DecisionSnapshot,
    suffix: &[TokenId],
) -> ProtocolResult<Option<PressMessage>> {
    let mut reader = Reader::new(suffix);
    if reader.peek() == Some(Token::End) {
        reader.expect_end()?;
        return Ok(None);
    }

    let visibility_position = reader.position();
    let visibility_token_value = reader.next("PUBLIC, PRIVATE, or END")?;
    let visibility =
        token_visibility(visibility_token_value).ok_or(ProtocolError::UnexpectedToken {
            position: visibility_position,
            found: visibility_token_value,
            expected: "PUBLIC, PRIVATE, or END",
        })?;
    let recipient = reader.next_power("recipient power")?;
    let act_position = reader.position();
    let act_token = reader.next("speech act")?;
    let act = match act_token {
        Token::Accept => SpeechAct::Accept,
        Token::Reject => SpeechAct::Reject,
        Token::Promise | Token::Request | Token::Propose => {
            let mut terms = Vec::new();
            while reader.peek() != Some(Token::End) {
                terms.push(parse_proposition(&mut reader)?);
            }
            match act_token {
                Token::Promise => SpeechAct::Promise(terms),
                Token::Request => SpeechAct::Request(terms),
                Token::Propose => SpeechAct::Propose(terms),
                _ => unreachable!(),
            }
        }
        found => {
            return Err(ProtocolError::UnexpectedToken {
                position: act_position,
                found,
                expected: "PROMISE, REQUEST, PROPOSE, ACCEPT, or REJECT",
            });
        }
    };
    reader.expect_end()?;

    let message = PressMessage {
        sender: snapshot.acting_power(),
        recipient,
        visibility,
        act,
    };
    let normalized = validate_press(snapshot, message.clone())?;
    if normalized != message {
        return Err(ProtocolError::NonCanonical { position: 3 });
    }
    Ok(Some(message))
}

fn parse_proposition(reader: &mut Reader<'_>) -> ProtocolResult<Proposition> {
    let actor = reader.next_power("term actor")?;
    if reader.peek() == Some(Token::Avoid) {
        reader.expect(Token::Avoid, "AVOID")?;
        return Ok(Proposition::Avoid {
            actor,
            location: reader.next_location("avoided location")?,
        });
    }
    let source = reader.next_location("embedded order source")?;
    Ok(Proposition::Order {
        actor,
        order: parse_order_tail(reader, source)?,
    })
}

struct Reader<'a> {
    tokens: &'a [TokenId],
    position: usize,
}

impl<'a> Reader<'a> {
    const fn new(tokens: &'a [TokenId]) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    const fn position(&self) -> usize {
        self.position
    }

    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.position).map(|token| token.token())
    }

    fn next(&mut self, expected: &'static str) -> ProtocolResult<Token> {
        let token = self
            .tokens
            .get(self.position)
            .copied()
            .ok_or(ProtocolError::Incomplete {
                position: self.position,
                expected,
            })?;
        self.position += 1;
        Ok(token.token())
    }

    fn expect(&mut self, expected_token: Token, expected: &'static str) -> ProtocolResult<()> {
        let position = self.position;
        let found = self.next(expected)?;
        if found != expected_token {
            return Err(ProtocolError::UnexpectedToken {
                position,
                found,
                expected,
            });
        }
        Ok(())
    }

    fn next_location(&mut self, expected: &'static str) -> ProtocolResult<Location> {
        let position = self.position;
        let found = self.next(expected)?;
        token_location(found).ok_or(ProtocolError::UnexpectedToken {
            position,
            found,
            expected,
        })
    }

    fn next_power(&mut self, expected: &'static str) -> ProtocolResult<Power> {
        let position = self.position;
        let found = self.next(expected)?;
        token_power(found).ok_or(ProtocolError::UnexpectedToken {
            position,
            found,
            expected,
        })
    }

    fn expect_end(&mut self) -> ProtocolResult<()> {
        self.expect(Token::End, "END")?;
        if self.position != self.tokens.len() {
            return Err(ProtocolError::TrailingTokens {
                position: self.position,
            });
        }
        Ok(())
    }
}

use game_engine::{CenterOwner, GameState, Location};

use crate::{
    DecisionSnapshot, GenerationMode, ProtocolError, ProtocolResult, Token, TokenId,
    token::{location_token, phase_token, power_token, year_token},
};

pub fn encode_observation(snapshot: &DecisionSnapshot) -> ProtocolResult<Vec<TokenId>> {
    if snapshot.diplomacy().is_some() {
        return Err(ProtocolError::DiplomaticContextUnavailable);
    }

    let state = snapshot.game_state();
    let mut tokens = Vec::with_capacity(95);
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

    tokens.push(match snapshot.mode() {
        GenerationMode::Movement => Token::Orders,
        GenerationMode::Adjustment => Token::Adjust,
        GenerationMode::Press => Token::Press,
    });
    debug_assert_eq!(tokens.len(), 95);
    Ok(tokens.into_iter().map(Token::id).collect())
}

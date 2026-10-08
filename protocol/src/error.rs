use crate::{GenerationMode, Token};
use thiserror::Error;

pub type ProtocolResult<T> = Result<T, ProtocolError>;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProtocolError {
    #[error("unknown token id {0}")]
    UnknownTokenId(u8),
    #[error("unknown token spelling {0:?}")]
    UnknownToken(String),
    #[error("the decision is for a finished game")]
    TerminalGame,
    #[error("{mode:?} generation is incompatible with the current game phase")]
    IncompatibleMode { mode: GenerationMode },
    #[error("press generation requires activation and visible negotiation state")]
    MissingPressDecision,
    #[error("press activation belongs to a different acting power")]
    WrongPressActivation,
    #[error("unexpected token {found:?} at generated position {position}; expected {expected}")]
    UnexpectedToken {
        position: usize,
        found: Token,
        expected: &'static str,
    },
    #[error("response ended at generated position {position}; expected {expected}")]
    Incomplete {
        position: usize,
        expected: &'static str,
    },
    #[error("tokens follow END at generated position {position}")]
    TrailingTokens { position: usize },
    #[error("response is not in canonical order at generated position {position}")]
    NonCanonical { position: usize },
    #[error("response is incomplete for the acting power")]
    IncompleteResponse,
    #[error("response contains a duplicate source or term")]
    Duplicate,
    #[error("response has the wrong typed variant for {mode:?} generation")]
    WrongResponseMode { mode: GenerationMode },
    #[error("response is semantically illegal: {0}")]
    IllegalResponse(String),
    #[error(
        "response budget {provided} cannot hold any legal response; at least {minimum} tokens are required"
    )]
    InsufficientBudget { provided: usize, minimum: usize },
    #[error("decoder prefix is invalid at generated position {position}: {reason}")]
    InvalidPrefix { position: usize, reason: String },
    #[error("the decoder is already terminal")]
    DecoderTerminal,
    #[error("the decoder has not reached END")]
    DecoderIncomplete,
    #[error("diplomatic context serialization requires the authoritative v0 record grammar")]
    DiplomaticContextUnavailable,
}

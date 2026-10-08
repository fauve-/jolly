mod decoder;
mod error;
mod observation;
mod response;
mod snapshot;
mod token;

pub use decoder::{Decoder, ResponseBudget};
pub use error::{ProtocolError, ProtocolResult};
pub use observation::encode_observation;
pub use response::{AdjustmentResponse, TypedResponse, encode_response, parse_response};
pub use snapshot::{DecisionSnapshot, GenerationMode, PressDecision};
pub use token::{
    PROTOCOL_VERSION, TOKEN_COUNT, TOKEN_MANIFEST_SHA256, Token, TokenId, TokenMask,
    tokens_from_ids,
};

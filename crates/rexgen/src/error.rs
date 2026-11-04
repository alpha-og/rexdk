use thiserror::Error;

use crate::{generator::GenerateError, parser::ParseError};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Parser(#[from] ParseError),

    #[error(transparent)]
    Generator(#[from] GenerateError),

    #[error("failed to deserialize API specification")]
    Json(#[from] serde_json::Error),
}

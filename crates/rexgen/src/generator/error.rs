use thiserror::Error;

use crate::generator::rust::RustGenerateError;

#[derive(Debug, Error)]
pub enum GenerateError {
    #[error(transparent)]
    Rust(#[from] RustGenerateError),
}

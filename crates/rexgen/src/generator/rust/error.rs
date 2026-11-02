use thiserror::Error;

use crate::generator::rust::registry::RegistryError;

#[derive(Debug, Error)]
pub enum RustGenerateError {
    #[error(transparent)]
    Registry(#[from] RegistryError),

    #[error("unsupported Rust type: {0}")]
    UnsupportedType(String),
}

use thiserror::Error;

use crate::generator::rust::registry::RegistryError;

#[derive(Debug, Error)]
pub enum RustGenerateError {
    #[error(transparent)]
    Registry(#[from] RegistryError),

    #[error("unsupported Rust type: {0}")]
    UnsupportedType(String),

    #[error("invalid Rust identifier `{name}` in {context}")]
    InvalidIdentifier { name: String, context: String },

    #[error("invalid namespace segment `{0}`")]
    InvalidNamespace(String),

    #[error("namespace segments `{existing}` and `{incoming}` collide as module `{canonical}`")]
    NamespaceCollision {
        existing: String,
        incoming: String,
        canonical: String,
    },

    #[error("duplicate binding `{name}` in module `{module}`")]
    DuplicateBinding { name: String, module: String },

    #[error("failed to parse generated Rust")]
    Syn(#[from] syn::Error),
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Unknown schema kind")]
    UnknownSchemaKind,
    #[error("Invalid schema")]
    InvalidSchema,
}

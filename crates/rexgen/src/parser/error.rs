use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Unknown schema kind")]
    UnknownSchemaKind,
    #[error("Invalid schema")]
    InvalidSchema,
    #[error("invalid method name `{0}`")]
    InvalidMethodName(String),
    #[error("duplicate method `{0}`")]
    DuplicateMethod(String),
}

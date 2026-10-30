#[derive(Debug)]
pub enum ParseError {
    UnknownSchemaKind,
    InvalidSchema,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSchemaKind => write!(f, "Unknown schema kind"),
            Self::InvalidSchema => write!(f, "Invalid schema"),
        }
    }
}

impl std::error::Error for ParseError {}

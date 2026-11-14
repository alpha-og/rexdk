use crate::ir::field::Field;

#[derive(Clone, Debug)]
pub struct Schema {
    pub description: Option<String>,
    pub schema_kind: SchemaKind,
    pub nullable: bool,
    pub anchor: Option<String>,
}

#[derive(Clone, Debug)]
pub enum SchemaKind {
    String,
    Integer,
    Number,
    Boolean,
    Null,
    Enum {
        base: Box<Schema>,
        values: Vec<serde_json::Value>,
    },
    Array {
        items: Box<Schema>,
        min_items: Option<u64>,
        max_items: Option<u64>,
    },
    Object {
        properties: Properties,
        additional_properties: AdditionalProperties,
    },
    Union {
        variants: Vec<Schema>,
    },
    Named {
        anchor: String,
    },
    Ref {
        anchor: String,
    },
    Unknown,
}

impl SchemaKind {
    /// Stable label for this kind, used to name generated variants.
    pub fn label(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Null => "null",
            Self::Enum { .. } => "enum",
            Self::Array { .. } => "array",
            Self::Object { .. } => "object",
            Self::Union { .. } => "union",
            Self::Named { .. } => "named",
            Self::Ref { .. } => "ref",
            Self::Unknown => "value",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Properties {
    Known(Vec<Field>),
    Unknown,
    None,
}

#[derive(Clone, Debug)]
pub enum AdditionalProperties {
    Disabled,
    Unknown,
    Schema(Box<Schema>),
}

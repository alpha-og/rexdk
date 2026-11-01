use crate::ir::field;

#[derive(Clone, Debug)]
pub struct Schema {
    pub description: Option<String>,
    pub schema_kind: SchemaKind,
    pub nullable: bool,
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
        min_items: Option<i64>,
        max_items: Option<i64>,
    },
    Object {
        properties: Vec<field::Field>,
        additional_properties: AdditionalProperties,
    },
    Unknown,
}

#[derive(Clone, Debug)]
pub enum AdditionalProperties {
    Disallowed,
    Allowed,
    Schema(Box<Schema>),
}

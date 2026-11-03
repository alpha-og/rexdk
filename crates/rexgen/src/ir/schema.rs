use crate::ir::field::Field;

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
        min_items: Option<u64>,
        max_items: Option<u64>,
    },
    Object {
        properties: Properties,
        additional_properties: AdditionalProperties,
    },
    Unknown,
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

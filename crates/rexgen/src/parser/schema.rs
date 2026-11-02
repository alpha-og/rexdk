use crate::{
    ir::schema::{Schema, SchemaKind},
    parser::error::ParseError,
};

impl TryFrom<&serde_json::Value> for Schema {
    type Error = ParseError;

    fn try_from(value: &serde_json::Value) -> Result<Self, Self::Error> {
        let mut nullable = false;
        if let Some(types) = value["type"].as_array() {
            nullable = types.iter().any(|t| t == "null")
        }

        let description = if let Some(description) = value.get("description") {
            if let Some(description_str) = description.as_str() {
                Some(String::from(description_str))
            } else {
                None
            }
        } else {
            None
        };
        let schema_kind = SchemaKind::try_from(value)?;

        Ok(Self {
            description,
            schema_kind,
            nullable,
        })
    }
}

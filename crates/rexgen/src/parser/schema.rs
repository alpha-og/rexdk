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

        let description = value
            .get("description")
            .and_then(serde_json::Value::as_str)
            .map(String::from);
        let schema_kind = match SchemaKind::try_from(value)? {
            SchemaKind::Union { mut variants } => {
                nullable |= variants
                    .iter()
                    .any(|variant| matches!(variant.schema_kind, SchemaKind::Null));
                variants.retain(|variant| !matches!(variant.schema_kind, SchemaKind::Null));

                match variants.len() {
                    0 => SchemaKind::Null,
                    1 => variants.remove(0).schema_kind,
                    _ => SchemaKind::Union { variants },
                }
            }
            other => other,
        };

        Ok(Self {
            description,
            schema_kind,
            nullable,
        })
    }
}

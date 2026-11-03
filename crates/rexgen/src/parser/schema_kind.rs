use crate::{
    ir::{
        field::Field,
        schema::{AdditionalProperties, Properties, Schema, SchemaKind},
    },
    parser::error::ParseError,
};

impl TryFrom<&serde_json::Value> for SchemaKind {
    type Error = ParseError;
    fn try_from(value: &serde_json::Value) -> Result<Self, Self::Error> {
        let base_kind = match value["type"].as_str() {
            Some(ty_str) => get_schema_kind_from_str(ty_str, value)?,
            None => value["type"]
                .as_array()
                .and_then(|types| {
                    types
                        .iter()
                        .find(|t| *t != "null")
                        .and_then(|ty| ty.as_str())
                })
                .map(|ty| get_schema_kind_from_str(ty, value))
                .unwrap_or(Ok(Self::Unknown))?,
        };
        match value.get("enum") {
            Some(serde_json::Value::Array(enum_values)) => Ok(SchemaKind::Enum {
                base: Box::new(Schema {
                    schema_kind: base_kind,
                    description: None,
                    nullable: false,
                }),
                values: enum_values.clone(),
            }),
            _ => Ok(base_kind),
        }
    }
}

fn get_schema_kind_from_str(
    str: &str,
    value: &serde_json::Value,
) -> Result<SchemaKind, ParseError> {
    let schema_kind = match str {
        "object" => {
            let required_fields = match value.get("required") {
                Some(serde_json::Value::Array(required_fields)) => {
                    required_fields.iter().filter_map(|f| f.as_str()).collect()
                }
                _ => vec![],
            };
            let mut properties = match value.get("properties") {
                Some(serde_json::Value::Object(properties)) => Properties::Known(
                    properties
                        .iter()
                        .map(|(name, schema)| {
                            let schema = Schema::try_from(schema)?;
                            let required = required_fields.iter().any(|f| f == name);
                            Ok(Field {
                                name: name.clone(),
                                schema,
                                required,
                            })
                        })
                        .collect::<Result<Vec<_>, ParseError>>()?,
                ),
                _ => Properties::Unknown,
            };

            let additional_properties = match value.get("additionalProperties") {
                Some(serde_json::Value::Bool(true)) | None => AdditionalProperties::Unknown,
                Some(serde_json::Value::Bool(false)) => AdditionalProperties::Disabled,
                Some(schema) if schema.is_object() => {
                    AdditionalProperties::Schema(Box::new(Schema::try_from(schema)?))
                }
                _ => {
                    return Err(ParseError::InvalidSchema);
                }
            };

            if let (AdditionalProperties::Disabled, Properties::Unknown) =
                (&additional_properties, &properties)
            {
                properties = Properties::None;
            };

            SchemaKind::Object {
                properties,
                additional_properties,
            }
        }
        "array" => {
            let items = Box::new(match value.get("items") {
                Some(items) => Schema::try_from(items)?,
                _ => {
                    return Err(ParseError::InvalidSchema);
                }
            });
            let min_items = match value.get("minItems") {
                Some(min_items) => min_items.as_u64(),
                _ => None,
            };
            let max_items = match value.get("maxItems") {
                Some(max_items) => max_items.as_u64(),
                _ => None,
            };

            SchemaKind::Array {
                items,
                min_items,
                max_items,
            }
        }
        "string" => SchemaKind::String,
        "integer" => SchemaKind::Integer,
        "number" => SchemaKind::Number,
        "boolean" => SchemaKind::Boolean,
        "null" => SchemaKind::Null,
        _ => SchemaKind::Unknown,
    };
    Ok(schema_kind)
}

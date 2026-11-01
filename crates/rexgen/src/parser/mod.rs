use crate::ir::Ir;
use crate::ir::method::Method;
use crate::ir::schema::{AdditionalProperties, Schema, SchemaKind};
use crate::spec::description::ApiDescription;
use crate::{ir::field::Field, spec::method::ApiMethod};

mod error;

use error::ParseError;

impl TryFrom<&ApiDescription> for Ir {
    type Error = ParseError;
    fn try_from(value: &ApiDescription) -> Result<Self, Self::Error> {
        let endpoints = value.http.clone();
        let methods = value
            .methods
            .iter()
            .map(|m| Method::try_from(m).unwrap())
            .collect();
        Ok(Self { endpoints, methods })
    }
}

impl TryFrom<&ApiMethod> for Method {
    type Error = ParseError;

    fn try_from(api_method: &ApiMethod) -> Result<Self, Self::Error> {
        let input_schema = match &api_method.input_schema {
            Some(schema) => Some(Schema::try_from(schema)?),
            None => None,
        };
        let output_schema = match &api_method.output_schema {
            Some(schema) => Some(Schema::try_from(schema)?),
            None => None,
        };

        Ok(Self {
            name: api_method.name.clone(),
            description: api_method.description.clone(),
            rpc_method: api_method.invoke.method.clone(),
            input_schema,
            output_schema,
        })
    }
}

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

impl TryFrom<&serde_json::Value> for SchemaKind {
    type Error = ParseError;
    fn try_from(value: &serde_json::Value) -> Result<Self, Self::Error> {
        let base_kind = match value["type"].as_str() {
            Some(ty_str) => get_schema_kind_from_str(ty_str, value)?,
            None => {
                if let Some(types) = value["type"].as_array() {
                    if let Some(ty) = types.iter().find(|t| *t != "null") {
                        if let Some(ty_str) = ty.as_str() {
                            get_schema_kind_from_str(ty_str, value)?
                        } else {
                            Self::Unknown
                        }
                    } else {
                        Self::Unknown
                    }
                } else {
                    Self::Unknown
                }
            }
            _ => {
                return Err(ParseError::UnknownSchemaKind);
            }
        };
        match value.get("enum") {
            Some(serde_json::Value::Array(enum_values)) => Ok(SchemaKind::Enum {
                base: Box::new(Schema {
                    schema_kind: base_kind,
                    description: None,
                    nullable: false,
                }),
                values: enum_values.iter().cloned().collect(),
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
            let properties = match value.get("properties") {
                Some(serde_json::Value::Object(properties)) => properties
                    .iter()
                    .map(|(name, schema)| {
                        let schema = Schema::try_from(schema).unwrap();
                        let required = required_fields.iter().any(|f| f == name);
                        Field {
                            name: name.clone(),
                            schema,
                            required,
                        }
                    })
                    .collect(),
                _ => vec![],
            };

            let additional_properties = match value.get("additionalProperties") {
                Some(serde_json::Value::Bool(true)) => AdditionalProperties::Allowed,
                Some(serde_json::Value::Bool(false)) => AdditionalProperties::Disallowed,
                Some(schema) if schema.is_object() => {
                    AdditionalProperties::Schema(Box::new(Schema::try_from(schema)?))
                }
                None => AdditionalProperties::Allowed,
                _ => {
                    return Err(ParseError::InvalidSchema);
                }
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
            let min_items = match value.get("min_items") {
                Some(min_items) => min_items.as_i64().take(),
                _ => None,
            };
            let max_items = match value.get("max_items") {
                Some(max_items) => max_items.as_i64().take(),
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

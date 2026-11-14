use std::collections::BTreeMap;

use crate::ir::{
    Definition, Ir, IrError,
    schema::{AdditionalProperties, Properties, Schema, SchemaKind},
};

impl Ir {
    pub(super) fn resolve(&mut self) -> Result<(), IrError> {
        let mut shapes: BTreeMap<(Vec<String>, String), String> = BTreeMap::new();
        let mut definitions = Vec::new();

        for method in &mut self.methods {
            let namespace = method.namespace.clone();
            if let Some(schema) = method.input_schema.as_mut() {
                Self::resolve_schema(schema, &namespace, &mut shapes, &mut definitions)?;
            }
            if let Some(schema) = method.output_schema.as_mut() {
                Self::resolve_schema(schema, &namespace, &mut shapes, &mut definitions)?;
            }
        }

        for method in &self.methods {
            if let Some(schema) = &method.input_schema {
                Self::validate_refs(schema, &method.namespace, &shapes)?;
            }
            if let Some(schema) = &method.output_schema {
                Self::validate_refs(schema, &method.namespace, &shapes)?;
            }
        }

        self.definitions = definitions;
        Ok(())
    }

    fn resolve_schema(
        schema: &mut Schema,
        namespace: &[String],
        shapes: &mut BTreeMap<(Vec<String>, String), String>,
        definitions: &mut Vec<Definition>,
    ) -> Result<(), IrError> {
        match &mut schema.schema_kind {
            SchemaKind::Object {
                properties,
                additional_properties,
            } => {
                if let Properties::Known(fields) = properties {
                    for field in fields {
                        Self::resolve_schema(&mut field.schema, namespace, shapes, definitions)?;
                    }
                }
                if let AdditionalProperties::Schema(schema) = additional_properties {
                    Self::resolve_schema(schema, namespace, shapes, definitions)?;
                }
            }
            SchemaKind::Array { items, .. } => {
                Self::resolve_schema(items, namespace, shapes, definitions)?;
            }
            SchemaKind::Enum { base, .. } => {
                Self::resolve_schema(base, namespace, shapes, definitions)?;
            }
            SchemaKind::Union { variants } => {
                for variant in variants {
                    Self::resolve_schema(variant, namespace, shapes, definitions)?;
                }
            }
            _ => {}
        }

        let Some(anchor) = schema.anchor.clone() else {
            return Ok(());
        };

        let shape = format!("{schema:?}");
        let key = (namespace.to_vec(), anchor.clone());
        if let Some(existing) = shapes.get(&key) {
            if existing != &shape {
                return Err(IrError::AnchorShapeMismatch(anchor));
            }
        } else {
            let body_kind = std::mem::replace(
                &mut schema.schema_kind,
                SchemaKind::Named {
                    anchor: anchor.clone(),
                },
            );
            let body = Schema {
                description: schema.description.clone(),
                schema_kind: body_kind,
                nullable: false,
                anchor: None,
            };
            shapes.insert(key, shape);
            definitions.push(Definition {
                namespace: namespace.to_vec(),
                anchor: anchor.clone(),
                schema: body,
            });
        }

        schema.anchor = None;
        schema.schema_kind = SchemaKind::Named { anchor };
        Ok(())
    }

    fn validate_refs(
        schema: &Schema,
        namespace: &[String],
        shapes: &BTreeMap<(Vec<String>, String), String>,
    ) -> Result<(), IrError> {
        match &schema.schema_kind {
            SchemaKind::Ref { anchor } => {
                if !shapes.contains_key(&(namespace.to_vec(), anchor.clone())) {
                    return Err(IrError::UnresolvedRef(anchor.clone()));
                }
            }
            SchemaKind::Object {
                properties,
                additional_properties,
            } => {
                if let Properties::Known(fields) = properties {
                    for field in fields {
                        Self::validate_refs(&field.schema, namespace, shapes)?;
                    }
                }
                if let AdditionalProperties::Schema(schema) = additional_properties {
                    Self::validate_refs(schema, namespace, shapes)?;
                }
            }
            SchemaKind::Array { items, .. } => Self::validate_refs(items, namespace, shapes)?,
            SchemaKind::Enum { base, .. } => Self::validate_refs(base, namespace, shapes)?,
            SchemaKind::Union { variants } => {
                for variant in variants {
                    Self::validate_refs(variant, namespace, shapes)?;
                }
            }
            _ => {}
        }

        Ok(())
    }
}

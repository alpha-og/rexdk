use crate::{
    generator::rust::{Generator, error::RustGenerateError, registry::TypeRegistry},
    ir::{
        field::Field,
        schema::{AdditionalProperties, Properties, Schema, SchemaKind},
    },
};
use heck::{ToSnakeCase, ToUpperCamelCase};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

impl Generator {
    pub(super) fn generate_primitive_type(
        schema: &Schema,
    ) -> Result<TokenStream, RustGenerateError> {
        let ty = match schema.schema_kind {
            SchemaKind::String => quote! { String },
            SchemaKind::Integer => quote! { i64 },
            SchemaKind::Number => quote! { f64 },
            SchemaKind::Boolean => quote! { bool },
            SchemaKind::Unknown => quote! { serde_json::Value },
            SchemaKind::Null => quote! {()},
            _ => unreachable!(),
        };

        Ok(ty)
    }

    pub(super) fn generate_type(
        registry: &mut TypeRegistry,
        schema: &Schema,
        name: &str,
    ) -> Result<TokenStream, RustGenerateError> {
        let ident = Self::generate_identifier(name, &format!("{:?}", schema))?;
        let ty = match &schema.schema_kind {
            SchemaKind::Array {
                items,
                min_items: _,
                max_items: _,
            } => Self::generate_type(registry, items, name).map(Self::generate_array_type)?,
            SchemaKind::Enum { base, values } => {
                Self::generate_enum_type(registry, name, schema.description.as_ref(), base, values)?
            }
            SchemaKind::Object {
                properties,
                additional_properties,
            } => {
                let description = Self::generate_description(schema.description.as_ref());
                let additional_attribute_ty = match additional_properties {
                    AdditionalProperties::Disabled => None,
                    AdditionalProperties::Unknown => Some(quote! {serde_json::Value}),
                    AdditionalProperties::Schema(schema) => Some(Self::generate_type(
                        registry,
                        schema,
                        &format!("{}Additional", name),
                    )?),
                };

                let definition = match properties {
                    Properties::Known(properties) => {
                        if additional_attribute_ty.is_some()
                            && properties
                                .iter()
                                .any(|property| property.name == "additional")
                        {
                            return Err(RustGenerateError::UnsupportedType(format!(
                                "{name} has a property named `additional` that collides with the flattened map"
                            )));
                        }

                        let struct_attributes = properties
                            .iter()
                            .map(|property| {
                                Self::generate_struct_attribute(registry, name, property)
                            })
                            .collect::<Result<Vec<_>, RustGenerateError>>()?;
                        let additional_struct_attribute = additional_attribute_ty.map(|ty| {
                            quote! {
                                #[serde(flatten)]
                                pub additional: std::collections::HashMap<String, #ty>
                            }
                        });

                        quote! {
                            pub struct #ident{
                                #(#struct_attributes,)*
                                #additional_struct_attribute
                            }
                        }
                    }
                    Properties::Unknown => {
                        let ty =
                            additional_attribute_ty.map_or(quote! {serde_json::Value}, |ty| ty);
                        quote! {pub struct #ident(pub std::collections::HashMap<String, #ty>);}
                    }
                    Properties::None => quote! {pub struct #ident{}},
                };

                let definition = quote! {
                    #description
                    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
                    #definition
                };

                registry.register(name, definition)?;
                quote! { #ident}
            }
            _ => Self::generate_primitive_type(schema)?,
        };
        if schema.nullable {
            Ok(quote! {Option<#ty>})
        } else {
            Ok(ty)
        }
    }

    fn generate_array_type(inner_type: TokenStream) -> TokenStream {
        quote! {
            Vec<#inner_type>
        }
    }

    fn generate_enum_type(
        registry: &mut TypeRegistry,
        name: &str,
        description: Option<&String>,
        base_type: &Schema,
        values: &Vec<serde_json::Value>,
    ) -> Result<TokenStream, RustGenerateError> {
        let ident = Self::generate_identifier(name, &format!("{:?}: [{:?}]", base_type, values))?;
        let description = Self::generate_description(description);
        match base_type.schema_kind {
            SchemaKind::String => {}
            _ => Err(RustGenerateError::UnsupportedType(format!(
                "{0:?}",
                base_type.schema_kind
            )))?,
        }
        let variants_tokens = values
            .iter()
            .map(|v| match v.as_str() {
                Some(v) => {
                    let variant =
                        Self::generate_identifier(&v.to_upper_camel_case(), "enum variant")?;
                    Ok(quote! {
                        #[serde(rename = #v)]
                        #variant
                    })
                }
                _ => Err(RustGenerateError::UnsupportedType(v.to_string())),
            })
            .collect::<Result<Vec<_>, RustGenerateError>>()?;
        let definition = quote! {
            #description
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            pub enum #ident{
                #(#variants_tokens,)*
            }
        };
        registry.register(name, definition)?;
        Ok(quote! {#ident})
    }

    fn generate_struct_attribute(
        registry: &mut TypeRegistry,
        struct_name: &str,
        field: &Field,
    ) -> Result<TokenStream, RustGenerateError> {
        let description = Self::generate_description(field.schema.description.as_ref());

        let attribute_name_ident = Self::generate_identifier(
            &format!("{}", &field.name.to_snake_case()),
            &format!("{:?}: {:?}", struct_name, field),
        )?;
        let serde_rename = (attribute_name_ident.to_string() != field.name).then(|| {
            let field_name = &field.name;
            quote! {
                #[serde(rename = #field_name)]
            }
        });
        let serde_omit_nullable =
            (!field.required).then(|| quote! {#[serde(skip_serializing_if = "Option::is_none")]});

        let mut attribute_type = Self::generate_type(
            registry,
            &field.schema,
            &format!("{}{}", struct_name, field.name.to_upper_camel_case()),
        )?;
        if !field.required {
            attribute_type = quote! { Option<#attribute_type> };
        }

        Ok(quote! {
            #description
            #serde_rename
            #serde_omit_nullable
            pub #attribute_name_ident: #attribute_type
        })
    }
}

use crate::{
    generator::rust::RustGenerator,
    ir::schema::{Schema, SchemaKind},
};
use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

impl RustGenerator {
    pub(super) fn generate_primitive_type(schema: &Schema) -> TokenStream {
        match schema.schema_kind {
            SchemaKind::String => quote! { String },
            SchemaKind::Integer => quote! { i64 },
            SchemaKind::Number => quote! { f64 },
            SchemaKind::Boolean => quote! { bool },
            SchemaKind::Unknown => quote! { serde_json::Value },
            _ => unreachable!(),
        }
    }

    pub(super) fn generate_type(&mut self, schema: &Schema, name: Ident) -> TokenStream {
        let ty = match &schema.schema_kind {
            SchemaKind::Array {
                items,
                min_items: _,
                max_items: _,
            } => {
                let schema_kind = self.generate_type(&items, name);
                quote! {
                    Vec<#schema_kind>
                }
            }
            SchemaKind::Enum { base, values } => {
                let description = if let Some(description) = &schema.description {
                    quote! {#[doc = #description]}
                } else {
                    quote! {}
                };
                match base.schema_kind {
                    SchemaKind::String => {}
                    _ => todo!(),
                }
                let variants_tokens = values.iter().map(|v| match v.as_str() {
                    Some(v) => {
                        let variant = format_ident!("{}", v.to_upper_camel_case());
                        quote! {
                            #[serde(rename = #v)]
                            #variant
                        }
                    }
                    _ => quote! {},
                });
                let definition = quote! {
                    #description
                    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
                    pub enum #name {
                        #(#variants_tokens,)*
                    }
                };
                self.registry.register(name.clone(), definition);
                quote! {#name}
            }
            SchemaKind::Object {
                properties,
                additional_properties: _,
            } => {
                let description = if let Some(description) = &schema.description {
                    quote! {#[doc = #description]}
                } else {
                    quote! {}
                };

                let fields = properties.iter().map(|p| {
                    let description = if let Some(description) = &p.schema.description {
                        quote! {#[doc = #description]}
                    } else {
                        quote! {}
                    };
                    let child_name = format_ident!("{}{}", name, p.name.to_upper_camel_case());

                    let field_name = format_ident!("{}", p.name.to_snake_case());
                    let field_name = if field_name == "type" || field_name == "where" {
                        format_ident!("r#{}", field_name)
                    } else {
                        field_name
                    };

                    let field_type = self.generate_type(&p.schema, child_name);
                    let field_type = if p.required {
                        field_type
                    } else {
                        quote! { Option<#field_type> }
                    };

                    quote! {
                        #description
                        pub #field_name : #field_type
                    }
                });

                let definition = quote! {
                    #description
                    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
                    pub struct #name {
                        #(#fields,)*
                    }
                };

                self.registry.register(name.clone(), definition);
                quote! { #name }
            }
            _ => Self::generate_primitive_type(schema),
        };
        if schema.nullable {
            quote! {Option<#ty>}
        } else {
            ty
        }
    }
}

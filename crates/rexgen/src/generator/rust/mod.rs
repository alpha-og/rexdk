use crate::{generator::rust::registry::TypeRegistry, ir::Ir};
use proc_macro2::{Span, TokenStream};
use quote::quote;

mod bindings;
mod error;
mod registry;
pub mod types;

pub use error::RustGenerateError;
use syn::Ident;

pub struct Generator {
    ir: Ir,
    registry: TypeRegistry,
}

impl Generator {
    pub fn new(ir: Ir) -> Self {
        Self {
            ir,
            registry: TypeRegistry::default(),
        }
    }

    fn generate_description(description: Option<&String>) -> TokenStream {
        description.map_or_else(|| quote! {}, |description| quote! {#[doc = #description]})
    }

    fn generate_identifier(name: &str) -> Ident {
        match name {
            "type" | "where" => Ident::new_raw(name, Span::call_site()),
            _ => Ident::new(name, Span::call_site()),
        }
    }
}

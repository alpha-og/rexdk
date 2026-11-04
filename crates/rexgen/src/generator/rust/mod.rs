use crate::{generator::rust::registry::TypeRegistry, ir::Ir};
use proc_macro2::TokenStream;
use quote::quote;

mod bindings;
mod error;
mod generated_file;
mod module;
mod namespace;
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

    pub fn generate_identifier(name: &str, context: &str) -> Result<Ident, RustGenerateError> {
        syn::parse_str::<Ident>(name) // fails for keywords and invalid names
            .or_else(|_| syn::parse_str::<Ident>(&format!("r#{name}"))) // fails for self/super/crate/Self
            .map_err(|_| RustGenerateError::InvalidIdentifier {
                name: name.to_owned(),
                context: context.to_owned(),
            })
    }
}

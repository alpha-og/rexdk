mod bindings;
mod error;
mod generated_file;
mod metadata;
mod module;
mod namespace;
mod registry;
pub mod types;

use std::path::Path;

use crate::{
    generator::rust::{module::Module, namespace::ModulePath, registry::TypeRegistry},
    ir::Ir,
    provenance::Provenance,
};
pub use error::RustGenerateError;
pub use generated_file::GeneratedFile;
use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

pub struct Generator {
    ir: Ir,
    root: Module,
}

impl Generator {
    pub fn new(ir: Ir) -> Self {
        Self {
            ir,
            root: Module::default(),
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

    pub fn generate(
        &mut self,
        base: &Path,
        provenance: &Provenance,
    ) -> Result<Vec<GeneratedFile>, RustGenerateError> {
        for definition in self.ir.definitions.clone() {
            let path = ModulePath::try_from_segments(&definition.namespace)?;
            let name = definition.anchor.to_upper_camel_case();
            let mut registry = TypeRegistry::default();
            Generator::generate_type(&mut registry, &definition.schema, &name)?;
            self.root.insert_types(&path, registry.flatten())?;
        }

        for method in self.ir.methods.clone() {
            let path = ModulePath::try_from_segments(&method.namespace)?;
            let binding = self.generate_binding(&method)?;
            self.root.insert(&path, binding)?;
        }

        self.root.declare(quote! {
            pub mod metadata;
        });

        let mut files = self.root.generate_files(base)?;
        files.push(metadata::generate(provenance, base)?);
        files.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(files)
    }
}

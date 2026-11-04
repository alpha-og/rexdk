use proc_macro2::Literal;
use quote::quote;
use std::path::Path;

use crate::{
    generator::rust::{GeneratedFile, error::RustGenerateError},
    provenance::Provenance,
};

pub(super) fn generate(
    provenance: &Provenance,
    dir: &Path,
) -> Result<GeneratedFile, RustGenerateError> {
    let schema_version = Literal::u32_unsuffixed(provenance.schema_version);
    let api_revision = Literal::u32_unsuffixed(provenance.api_revision);
    let rex_version = Literal::string(&provenance.rex_version);
    let server_commit = Literal::string(&provenance.server_commit);
    let server_dirty = provenance.server_dirty;
    let schema_sha256 = Literal::string(&provenance.schema_sha256);

    let definition = quote! {
        /// Metadata of the API schema these bindings were generated from.
        #[derive(Debug, Clone, Copy)]
        pub struct Metadata {
            pub schema_version: u32,
            pub api_revision: u32,
            pub rex_version: &'static str,
            pub server_commit: &'static str,
            pub server_dirty: bool,
            pub schema_sha256: &'static str,
        }

        pub const METADATA: Metadata = Metadata {
            schema_version: #schema_version,
            api_revision: #api_revision,
            rex_version: #rex_version,
            server_commit: #server_commit,
            server_dirty: #server_dirty,
            schema_sha256: #schema_sha256,
        };

        pub const SCHEMA_VERSION: u32 = #schema_version;
        pub const API_REVISION: u32 = #api_revision;
        pub const REX_VERSION: &'static str = #rex_version;
        pub const SERVER_COMMIT: &'static str = #server_commit;
        pub const SERVER_DIRTY: bool = #server_dirty;
        pub const SCHEMA_SHA256: &'static str = #schema_sha256;
    };
    let ast = syn::parse2::<syn::File>(definition)?;

    Ok(GeneratedFile {
        path: dir.join("metadata.rs"),
        contents: prettyplease::unparse(&ast),
    })
}

//! Generates Rust SDK bindings from a rex API description (`GET /api/help` JSON).
//!
//! This crate is pure: it takes the spec as a string and returns source code as
//! a string. Reading the spec and writing the output are the caller's job.

mod error;
mod generator;
mod ir;
mod parser;
mod spec;

pub use error::Error;
pub use generator::GenerateError;
pub use parser::ParseError;

/// Generates Rust bindings from the JSON text of a rex API description.
pub fn generate(spec_json: &str) -> Result<String, Error> {
    let description: spec::description::ApiDescription = serde_json::from_str(spec_json)?;
    let ir = ir::Ir::try_from(&description)?;
    let tokens = generator::rust::Generator::new(ir)
        .generate_bindings()
        .map_err(GenerateError::from)?;
    let file = syn::parse2::<syn::File>(tokens)?;
    Ok(prettyplease::unparse(&file))
}

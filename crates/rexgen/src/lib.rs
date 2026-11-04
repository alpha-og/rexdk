//! Generates Rust SDK bindings from a rex API description (`GET /api/help` JSON).
//!
//! This crate is pure: it takes the spec as a string and returns generated
//! source files as a list of paths and contents. Reading the spec and writing
//! the output are the caller's job.

mod error;
mod generator;
mod ir;
mod parser;
mod spec;

use std::path::Path;

pub use error::Error;
pub use generator::GenerateError;
pub use generator::rust::GeneratedFile;
pub use parser::ParseError;

/// Generates Rust bindings from the JSON text of a rex API description.
///
/// Paths in the returned files are rooted at `base`.
pub fn generate(spec_json: &str, base: &Path) -> Result<Vec<GeneratedFile>, Error> {
    let description: spec::description::ApiDescription = serde_json::from_str(spec_json)?;
    let ir = ir::Ir::try_from(&description)?;
    let files = generator::rust::Generator::new(ir)
        .generate(base)
        .map_err(GenerateError::from)?;
    Ok(files)
}

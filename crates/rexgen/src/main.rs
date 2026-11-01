use std::{fs::File, io::Read, path::Path};

use crate::{ir::Ir, spec::description::ApiDescription};

mod generator;
mod ir;
mod parser;
mod spec;

const API_DIRECTORY: &str = "api";
const API_SPEC_FILENAME: &str = "rex-api";

const API_GENERATED_DIRECTORY: &str = "rexdk/src";
const API_GENERATED_FILENAME: &str = "generated";

fn main() -> anyhow::Result<()> {
    let api_spec_filepath_raw = format!("./{API_DIRECTORY}/{API_SPEC_FILENAME}.json");
    let api_spec_filepath = Path::new(&api_spec_filepath_raw);

    let mut api_spec_file = File::open(&api_spec_filepath)?;

    let mut raw_api_spec = String::new();
    api_spec_file.read_to_string(&mut raw_api_spec)?;

    let api_spec: ApiDescription = serde_json::from_str(&raw_api_spec)?;
    let ir = Ir::try_from(&api_spec)?;
    let mut generator = generator::rust::RustGenerator::new(ir);
    let tokens = generator.generate_bindings();
    let file = syn::parse2::<syn::File>(tokens).map_err(|e| {
        eprintln!("Parse error: {}", e);
        eprintln!("Span: {:?}", e.span());
        e
    })?;
    let formatted = prettyplease::unparse(&file);

    let api_generated_filepath_raw =
        format!("./{API_GENERATED_DIRECTORY}/{API_GENERATED_FILENAME}.rs");
    let api_generated_filepath = Path::new(&api_generated_filepath_raw);
    dbg!(api_generated_filepath);
    std::fs::write("generated.rs", formatted)?;

    Ok(())
}

use std::{fs::File, io::Read, path::Path};

use rexgen::{error::Error, generator, ir::Ir, spec::description::ApiDescription};

const API_DIRECTORY: &str = "api";
const API_SPEC_FILENAME: &str = "rex-api";

const API_GENERATED_DIRECTORY: &str = "crates/rexdk/src";
const API_GENERATED_FILENAME: &str = "generated";

fn main() -> Result<(), Error> {
    let api_spec_filepath_raw = format!("./{API_DIRECTORY}/{API_SPEC_FILENAME}.json");
    let api_spec_filepath = Path::new(&api_spec_filepath_raw);

    let mut api_spec_file = File::open(&api_spec_filepath)?;

    let mut schema = String::new();
    api_spec_file.read_to_string(&mut schema)?;

    let source_code = generate(schema)?;

    let api_generated_filepath_raw =
        format!("./{API_GENERATED_DIRECTORY}/{API_GENERATED_FILENAME}.rs");
    let api_generated_filepath = Path::new(&api_generated_filepath_raw);
    dbg!(api_generated_filepath);
    std::fs::write(api_generated_filepath, source_code)?;

    Ok(())
}

fn generate(schema: String) -> Result<String, Error> {
    let api_description: ApiDescription = serde_json::from_str(&schema)?;
    let ir = Ir::try_from(&api_description)?;
    let mut generator = generator::rust::Generator::new(ir);
    let tokens = generator
        .generate_bindings()
        .map_err(|err| Error::Generator(generator::GenerateError::Rust(err)))?;
    let ast = syn::parse2::<syn::File>(tokens)?;
    let source_code = prettyplease::unparse(&ast);
    Ok(source_code)
}

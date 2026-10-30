use std::{fs::File, io::Read, path::Path};

use anyhow::Result;

use crate::spec::description::ApiDescription;

mod generator;
mod ir;
mod parser;
mod spec;

const API_DIRECTORY: &str = "api";
const API_SPEC_FILENAME: &str = "rex-api";

const API_GENERATED_DIRECTORY: &str = "rexdk/src";
const API_GENERATED_FILENAME: &str = "generated";

fn main() -> Result<()> {
    let api_spec_filepath_raw = format!("./{API_DIRECTORY}/{API_SPEC_FILENAME}.json");
    let api_spec_filepath = Path::new(&api_spec_filepath_raw);

    let mut api_spec_file = File::open(&api_spec_filepath)?;

    let mut raw_api_spec = String::new();
    api_spec_file.read_to_string(&mut raw_api_spec)?;

    let api_spec: ApiDescription = serde_json::from_str(&raw_api_spec)?;
    let _generated_code = generate(&api_spec);

    let _api_generated_filepath_raw =
        format!("./{API_GENERATED_DIRECTORY}/{API_GENERATED_FILENAME}.rs");
    let _api_generated_filepath = Path::new(&api_spec_filepath_raw);

    // let generated_file = File::create_new(api_generated_filepath)?;

    Ok(())
}

fn generate(api_spec: &ApiDescription) {
    api_spec.methods.iter().for_each(|method| {
        let method = ir::method::Method::try_from(method).unwrap();
        dbg!(method);
    });
}

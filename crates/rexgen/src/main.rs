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
    dbg!(ir);

    let _api_generated_filepath_raw =
        format!("./{API_GENERATED_DIRECTORY}/{API_GENERATED_FILENAME}.rs");
    let _api_generated_filepath = Path::new(&api_spec_filepath_raw);

    // let generated_file = File::create_new(api_generated_filepath)?;

    Ok(())
}

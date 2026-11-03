use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let default = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../api/rex-api.json");
    let path = env::args().nth(1).map(PathBuf::from).unwrap_or(default);

    let spec = fs::read_to_string(&path)?;
    let source = rexgen::generate(&spec)?;
    print!("{source}");
    Ok(())
}

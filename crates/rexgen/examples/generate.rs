use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let default = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../api/rex-api.json");
    let path = env::args().nth(1).map(PathBuf::from).unwrap_or(default);
    let out = env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("bindings"));

    let spec = fs::read_to_string(&path)?;
    let files = rexgen::generate(&spec, &out)?;

    for file in files {
        if let Some(parent) = file.path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&file.path, file.contents)?;
        println!("{}", file.path.display());
    }

    Ok(())
}

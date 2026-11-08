use std::path::Path;

fn file<'a>(files: &'a [rexgen::GeneratedFile], path: &str) -> &'a str {
    files
        .iter()
        .find(|file| file.path == Path::new(path))
        .unwrap_or_else(|| panic!("missing generated file {path}"))
        .contents
        .as_str()
}

fn provenance() -> rexgen::Provenance {
    rexgen::Provenance {
        schema_version: 1,
        api_revision: 10,
        rex_version: "0.0.1793645657-gd018d2".to_owned(),
        server_commit: "d018d2de8cc31281265906a4648ec9cae8d2f4c5".to_owned(),
        server_dirty: false,
        schema_sha256: "c3ac62d8d918b6a8f8bbf573fdc365d4842b7bab21e0ddb9a9b88c5746bd2833"
            .to_owned(),
    }
}

#[test]
fn generates_modular_bindings() {
    let spec = include_str!("../../../api/rex-api.json");
    let files =
        rexgen::generate(spec, Path::new("bindings"), &provenance()).expect("generate bindings");

    for file in &files {
        syn::parse_file(&file.contents)
            .unwrap_or_else(|error| panic!("{} is invalid Rust: {error}", file.path.display()));
    }

    assert_eq!(
        file(&files, "bindings/mod.rs"),
        "pub mod metadata;\npub mod block;\npub mod block_creator;\npub mod client;\npub mod data;\npub mod help;\npub mod hosts;\npub mod server;\npub mod session;\npub mod target;\n"
    );
    assert!(file(&files, "bindings/session/mod.rs").contains("pub async fn r#move("));
    assert!(file(&files, "bindings/client/mod.rs").contains("pub struct ListParams"));
    assert!(file(&files, "bindings/session/mod.rs").contains("pub struct ListParams"));
    assert!(
        file(&files, "bindings/server/apple/mod.rs")
            .contains("use crate::runtime::{Client, Error};")
    );

    let metadata = file(&files, "bindings/metadata.rs");
    assert!(metadata.contains("pub const API_REVISION: u32 = 10;"));
    assert!(metadata.contains("pub const SERVER_DIRTY: bool = false;"));
    assert!(metadata.contains(
        "pub const SCHEMA_SHA256: &'static str = \"c3ac62d8d918b6a8f8bbf573fdc365d4842b7bab21e0ddb9a9b88c5746bd2833\";"
    ));
}

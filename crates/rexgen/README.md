# rexgen

A Rust binding generator for the Rex API.

[Rex website](https://www.superlogical.com/rex) · [Rex documentation](https://www.superlogical.com/rex/docs) · [Repository](https://github.com/alpha-og/rexdk-rs)

`rexgen` converts a Rex API description into a tree of Rust modules and records
the provenance of the schema that produced it. The library is pure: it accepts
the API description as JSON text and returns the generated file paths and
contents. Reading the description and writing the output are the caller's
responsibility.

The `rexgen` binary adds schema fetching, lock-file verification, and
staleness checks.

## Library

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec = std::fs::read_to_string("api/rex-api.json")?;
    let files = rexgen::generate(
        &spec,
        std::path::Path::new("bindings"),
        &rexgen::Provenance::default(),
    )?;

    for file in files {
        if let Some(parent) = file.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file.path, file.contents)?;
    }
    Ok(())
}
```

`generate` returns the binding files only, rooted at the given directory. The
CLI also prepends a header containing the API revision and schema hash to every
file, and passes the lock-file provenance so it is embedded in
`bindings/metadata.rs`.

## CLI

The binary is enabled by the default `cli` feature.

```sh
# Regenerate crates/rexdk/src/bindings/ from the checked-in spec.
cargo run -p rexgen -- generate

# Fail instead of writing when the checked-in bindings are stale.
cargo run -p rexgen -- generate --check

# Fetch the spec and provenance from a running Rex server.
cargo run -p rexgen -- update-schema
```

### `generate`

| Option | Default | Description |
| --- | --- | --- |
| `--spec` | `api/rex-api.json` | API description to read. |
| `--lock` | `api/rex-api.lock.json` | Provenance file used to verify the spec. |
| `--out` | `crates/rexdk/src/bindings` | Output directory for the generated module tree. |
| `--check` | off | Compares instead of writing, and exits non-zero when the output is stale or contains unexpected files. |

Before generating, `generate` verifies the spec's SHA-256 against the lock file.
If the spec changed without a corresponding lock update, generation fails.

### `update-schema`

| Option | Default | Description |
| --- | --- | --- |
| `--spec` | `api/rex-api.json` | Output path for the API description. |
| `--lock` | `api/rex-api.lock.json` | Output path for the provenance file. |
| `--socket` | default transport | Rex server socket path to connect to. |

`update-schema` connects to a running Rex server and calls `help.all` and
`server.version`. It writes the API description and a lock file containing:

- the lock schema version,
- the API revision,
- the Rex version and server commit,
- whether the server was built from a dirty tree,
- the SHA-256 of the description.

## Generation pipeline

1. Parse the API description into typed spec structs.
2. Convert the spec into an intermediate representation of endpoints, methods,
   and types.
3. Generate Rust tokens for types and method functions.
4. Format the output with `prettyplease`.

Unsupported schema shapes are reported as `GenerateError` rather than emitted
as invalid Rust.

## Example

```sh
cargo run -p rexgen --example generate
```

The example reads `api/rex-api.json` and writes the generated module tree to
`bindings/`. Pass a different spec path as the first argument and a different
output directory as the second.

/// Provenance of the API schema a binding set was generated from.
///
/// Mirrors the fields of `api/rex-api.lock.json`. The generator embeds these
/// values into the generated bindings so they are available at compile time.
#[derive(Debug, Clone, Default)]
pub struct Provenance {
    pub schema_version: u32,
    pub api_revision: u32,
    pub rex_version: String,
    pub server_commit: String,
    pub server_dirty: bool,
    pub schema_sha256: String,
}

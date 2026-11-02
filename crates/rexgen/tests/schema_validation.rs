#[test]
fn rex_schema_is_valid() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../api/rex-api.json")).unwrap();

    jsonschema::meta::validate(&schema).expect("Rex schema should be a valid JSON Schema");
}

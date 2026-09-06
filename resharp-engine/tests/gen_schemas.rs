mod common;
use common::schemas::*;
use schemars::JsonSchema;
use std::path::PathBuf;

fn write_schema<T: JsonSchema>(name: &str) {
    let schema = schemars::schema_for!(T);
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../data/schemas")
        .join(format!("{name}.schema.json"));
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, serde_json::to_string_pretty(&schema).unwrap()).unwrap();
}

#[test]
fn gen_schemas() {
    write_schema::<EngineFile>("engine");
    write_schema::<InternalFile>("internal");
    write_schema::<CapturesFile>("captures");
    write_schema::<PrefixFile>("prefix");
    write_schema::<RevNullsFile>("rev_nulls");
    write_schema::<AutoHardenFile>("auto_harden");
    write_schema::<QuadraticFile>("quadratic");
    write_schema::<DerivFile>("deriv");
}

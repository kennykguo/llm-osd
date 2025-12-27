// ABOUTME: emits a json schema for the actionplan protocol types to stdout.
// ABOUTME: intended for use with constrained decoding and external validators.

fn main() {
    let schema = schemars::schema_for!(llm_os_common::ActionPlan);
    let json = serde_json::to_string_pretty(&schema).expect("serialize schema");
    println!("{json}");
}



//! GH#592: a call that names a library free function must not be generated as a method of the
//! client the default call constructs. The call opts out with an empty `client_factory` in its
//! own override table; a sibling fixture on the default call keeps its client.

use super::E2eCodegen;
use crate::e2e::fixture::{Fixture, FixtureGroup};

const CONFIG: &str = r#"
[workspace]
languages = ["python", "node", "rust"]

[[crates]]
name = "mini-greet"
sources = ["src/lib.rs"]

[crates.e2e]
fixtures = "fixtures"
output = "e2e"

[crates.e2e.call]
function = "greet"
module = "mini_greet"
result_var = "result"
async = true
args = [{ name = "name", field = "input.name", type = "string" }]

[crates.e2e.call.overrides.python]
client_factory = "create_greeter"
[crates.e2e.call.overrides.node]
client_factory = "createGreeter"
[crates.e2e.call.overrides.rust]
client_factory = "create_greeter"

[crates.e2e.calls.known_names]
function = "known_names"
module = "mini_greet"
result_is_array = true
args = [{ name = "prefix", field = "input.prefix", type = "string", optional = true }]

[crates.e2e.calls.known_names.overrides.python]
client_factory = ""
[crates.e2e.calls.known_names.overrides.node]
client_factory = ""
[crates.e2e.calls.known_names.overrides.rust]
client_factory = ""
"#;

fn fixtures() -> Vec<FixtureGroup> {
    let client_call: Fixture = serde_json::from_value(serde_json::json!({
        "id": "greet_alice",
        "description": "Greeting a name through the client",
        "input": {"name": "alice"},
        "mock_response": {"status": 200, "body": "hello alice"},
        "assertions": [{"type": "not_error"}]
    }))
    .expect("fixture parses");
    let free_call: Fixture = serde_json::from_value(serde_json::json!({
        "id": "known_names_by_prefix",
        "description": "Listing the known names needs no client",
        "call": "known_names",
        "input": {"prefix": "al"},
        "assertions": [{"type": "not_error"}, {"type": "count_min", "value": 1}]
    }))
    .expect("fixture parses");
    vec![FixtureGroup {
        category: "greet".to_string(),
        fixtures: vec![client_call, free_call],
    }]
}

fn render(codegen: &dyn E2eCodegen) -> String {
    render_config(codegen, CONFIG)
}

fn render_config(codegen: &dyn E2eCodegen, config: &str) -> String {
    let cfg: crate::core::config::NewAlefConfig = toml::from_str(config).expect("config parses");
    let e2e = cfg.crates[0].e2e.clone().expect("e2e config");
    let resolved = cfg.resolve().expect("config resolves").remove(0);
    let files = codegen
        .generate(&fixtures(), &e2e, &resolved, &[], &[], &[], &[])
        .expect("e2e codegen succeeds");
    let test = files
        .iter()
        .find(|f| {
            let name = f.path.to_string_lossy();
            name.contains("greet") && !name.contains("conftest") && !name.contains("conformance")
        })
        .unwrap_or_else(|| {
            panic!(
                "no greet test file in {:?}",
                files.iter().map(|f| &f.path).collect::<Vec<_>>()
            )
        });
    test.content.clone()
}

#[test]
fn python_free_function_call_is_not_a_client_method() {
    let out = render(&super::python::PythonE2eCodegen);
    assert!(out.contains("known_names("), "{out}");
    assert!(!out.contains("client.known_names("), "{out}");
    assert!(
        out.contains("client.greet("),
        "client call must keep its client:\n{out}"
    );
    assert!(out.contains("import") && out.contains("known_names"), "{out}");
    assert!(
        out.contains("from mini_greet import") && out.contains("create_greeter"),
        "{out}"
    );
}

#[test]
fn node_free_function_call_is_not_a_client_method() {
    let out = render(&super::typescript::TypeScriptCodegen);
    assert!(out.contains("knownNames("), "{out}");
    assert!(!out.contains("client.knownNames("), "{out}");
    assert!(
        !out.contains("fixtures/known_names_by_prefix"),
        "a free-function fixture needs no mock server:\n{out}"
    );
    assert!(
        out.contains("client.greet("),
        "client call must keep its client:\n{out}"
    );
    assert!(
        out.lines()
            .any(|l| l.starts_with("import {") && l.contains("knownNames") && l.contains("createGreeter")),
        "both the factory and the free function must be imported:\n{out}"
    );
}

#[test]
fn rust_free_function_call_is_not_a_client_method() {
    let out = render(&super::rust::RustE2eCodegen);
    assert!(out.contains("use mini_greet::known_names;"), "{out}");
    assert!(out.contains("known_names("), "{out}");
    assert!(!out.contains("client.known_names("), "{out}");
    assert!(
        out.contains("client.greet("),
        "client call must keep its client:\n{out}"
    );
}

/// Negative control: without the empty override the named call still inherits the default
/// call's client, so the assertions above are wired to the opt-out and not to something else.
#[test]
fn a_named_call_without_the_empty_override_keeps_the_default_client() {
    let config = CONFIG.replace("client_factory = \"\"", "");
    for (codegen, call) in [
        (
            &super::python::PythonE2eCodegen as &dyn E2eCodegen,
            "client.known_names(",
        ),
        (&super::typescript::TypeScriptCodegen, "client.knownNames("),
        (&super::rust::RustE2eCodegen, "client.known_names("),
    ] {
        let out = render_config(codegen, &config);
        assert!(out.contains(call), "expected {call} without the opt-out:\n{out}");
    }
}

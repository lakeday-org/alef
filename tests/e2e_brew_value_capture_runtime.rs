#![cfg(unix)]

use alef::core::config::NewAlefConfig;
use alef::e2e::codegen::E2eCodegen;
use alef::e2e::codegen::brew::BrewCodegen;
use alef::e2e::fixture::{Assertion, Fixture, FixtureGroup};
use serde_json::{Value, json};
use std::process::{Command, Output};

fn config() -> NewAlefConfig {
    toml::from_str(
        r#"
[workspace]
languages = ["python"]
[[crates]]
name = "sample"
sources = ["src/lib.rs"]
[crates.e2e]
fixtures = "fixtures"
output = "e2e"
languages = ["brew"]
[crates.e2e.call]
function = "convert"
module = "sample_cli"
result_var = "result"
"#,
    )
    .unwrap()
}

fn generate(assertion_type: &str, field: &str, value: Value) -> String {
    generate_assertions(vec![Assertion {
        assertion_type: assertion_type.into(),
        field: Some(field.into()),
        value: Some(value),
        ..Assertion::default()
    }])
}

fn generate_assertions(assertions: Vec<Assertion>) -> String {
    let config = config();
    let fixture = Fixture {
        id: "capture".into(),
        category: Some("capture".into()),
        assertions,
        ..Fixture::default()
    };
    let resolved = config.clone().resolve().unwrap().remove(0);
    let files = BrewCodegen
        .generate(
            &[FixtureGroup {
                category: "capture".into(),
                fixtures: vec![fixture],
            }],
            config.crates[0].e2e.as_ref().unwrap(),
            &resolved,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
    files
        .into_iter()
        .find(|file| file.path.ends_with("test_capture.sh"))
        .unwrap()
        .content
}

fn execute(script: &str, payload: &str) -> (Output, usize) {
    let directory = tempfile::tempdir().unwrap();
    let category = directory.path().join("category.sh");
    let log = directory.path().join("assertions.log");
    std::fs::write(&category, script).unwrap();
    let output = Command::new("bash")
        .arg("-c")
        .arg(include_str!("fixtures/brew_value_capture_runtime.sh"))
        .env("CATEGORY_SCRIPT", category)
        .env("JSON_PAYLOAD", payload)
        .env("ASSERTION_LOG", &log)
        .current_dir(directory.path())
        .output()
        .expect("Brew runtime regressions require Bash and real jq");
    let count = std::fs::read_to_string(log).unwrap_or_default().lines().count();
    (output, count)
}

fn assert_status(script: &str, payload: &str, success: bool, expected_assertions: usize) {
    let (output, assertions) = execute(script, payload);
    assert_eq!(
        output.status.success(),
        success,
        "{}\n{script}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        assertions, expected_assertions,
        "assertion coverage must be non-vacuous"
    );
}

#[test]
fn emitted_equals_preserves_exact_json_scalar_bytes() {
    let values = [
        json!(""),
        json!("plain"),
        json!("one\n"),
        json!("many\n\n\n"),
        json!("."),
        json!("sentinel.\n"),
        json!("newline\n."),
        json!("'quoted' \\\" 東京\n\n"),
        json!(0),
        json!(false),
    ];
    assert_eq!(values.len(), 10);
    for value in values {
        assert_status(
            &generate("equals", "value", value.clone()),
            &json!({"value": value}).to_string(),
            true,
            1,
        );
    }
}

#[test]
fn emitted_equals_rejects_changed_bytes_and_extraction_errors_inside_if() {
    assert_status(
        &generate("equals", "value", json!("one\n")),
        r#"{"value":"one"}"#,
        false,
        1,
    );
    for payload in ["", " \n", "{", "{} {}"] {
        let (output, assertions) = execute(&generate("equals", "value", json!("")), payload);
        assert!(!output.status.success());
        assert_eq!(assertions, 0);
        assert!(String::from_utf8_lossy(&output.stderr).contains("cannot extract JSON value"));
    }
    let (output, assertions) = execute(&generate("equals", "value[", json!("")), r#"{"value":""}"#);
    assert!(!output.status.success());
    assert_eq!(assertions, 0);
    assert!(String::from_utf8_lossy(&output.stderr).contains("compile error"));
}

#[test]
fn other_string_assertions_use_the_same_lossless_capture() {
    for (kind, expected, payload, success) in [
        ("is_empty", json!(null), json!("\n"), false),
        ("is_empty", json!(null), json!(null), true),
        ("contains", json!("null"), json!(null), true),
        ("not_empty", json!(null), json!("\n"), true),
        ("ends_with", json!("\n"), json!("text\n"), true),
        ("min_length", json!(2), json!("x\n"), true),
        ("max_length", json!(1), json!("x\n"), false),
    ] {
        assert_status(
            &generate(kind, "value", expected),
            &json!({"value": payload}).to_string(),
            success,
            0,
        );
    }
}

#[test]
fn previous_capture_fails_for_trailing_newlines_for_the_expected_reason() {
    for value in ["one\n", "many\n\n"] {
        let script = generate("equals", "value", json!(value));
        let begin = script.find("  local val_value\n").unwrap();
        let end = script[begin..].find("  assert_equals").unwrap() + begin;
        let legacy = "  local val_value\n  val_value=$(echo \"$output\" | jq -r '.value')\n";
        let script = [&script[..begin], legacy, &script[end..]].concat();
        let (output, assertions) = execute(&script, &json!({"value": value}).to_string());
        assert!(!output.status.success());
        assert_eq!(assertions, 1);
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "exact mismatch");
    }
}

#[test]
fn first_equals_failure_cannot_be_masked_by_a_later_success() {
    let assertions = ["first", "second"].map(|field| Assertion {
        assertion_type: "equals".into(),
        field: Some(field.into()),
        value: Some(json!("expected")),
        ..Assertion::default()
    });
    let script = generate_assertions(assertions.into());
    let payload = json!({"first": "wrong", "second": "expected"}).to_string();
    assert_status(&script, &payload, false, 1);
    let unguarded = script.replace(" || return \"$?\"", "");
    assert_ne!(unguarded, script);
    assert_status(&unguarded, &payload, true, 2);
}

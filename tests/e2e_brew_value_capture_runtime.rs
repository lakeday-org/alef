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
        .find(|file| file.path.ends_with("test_capture.bats"))
        .unwrap()
        .content
}

fn execute(script: &str, payload: &str) -> (Output, usize) {
    let directory = tempfile::tempdir().unwrap();
    let category = directory.path().join("category.bats");
    let log = directory.path().join("assertions.log");
    let instrumented = script
        .lines()
        .map(|line| {
            if line.starts_with("  [[ \"$val_") && line.contains(" == ") {
                format!("  printf 'assertion\\n' >> \"$ASSERTION_LOG\"\n{line}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let stub = directory.path().join("sample_cli");
    std::fs::write(&stub, "#!/usr/bin/env bash\nprintf '%s' \"$JSON_PAYLOAD\"\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(&category, instrumented).unwrap();
    let mut output = Command::new("bats")
        .arg(&category)
        .env("JSON_PAYLOAD", payload)
        .env("ASSERTION_LOG", &log)
        .env("BATS_LIB_PATH", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/lib"))
        .env(
            "PATH",
            format!("{}:{}", directory.path().display(), std::env::var("PATH").unwrap()),
        )
        .current_dir(directory.path())
        .output()
        .expect("Bats and real jq are required");
    output.stderr.extend_from_slice(&output.stdout);
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
            usize::from(matches!(kind, "contains" | "ends_with")),
        );
    }
}

#[test]
fn previous_capture_fails_for_trailing_newlines_for_the_expected_reason() {
    for value in ["one\n", "many\n\n"] {
        let script = generate("equals", "value", json!(value));
        let begin = script.find("  local val_value\n").unwrap();
        let end = script[begin..].find("  [[ ").unwrap() + begin;
        let legacy = "  local val_value\n  val_value=$(echo \"$output\" | jq -r '.value')\n";
        let script = [&script[..begin], legacy, &script[end..]].concat();
        let (output, assertions) = execute(&script, &json!({"value": value}).to_string());
        assert!(!output.status.success());
        assert_eq!(assertions, 1);
        assert!(String::from_utf8_lossy(&output.stdout).contains("not ok 1 capture"));
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
    let native_assertion = "  [[ \"$val_first\" == 'expected' ]]";
    let ignored_failure = script.replace(native_assertion, &format!("{native_assertion} || true"));
    assert_ne!(ignored_failure, script);
    assert_status(&ignored_failure, &payload, true, 2);
}

#[test]
fn projected_stream_preserves_separators_and_data_newlines() {
    let cases = [
        (json!([]), ""),
        (json!(["single"]), "single"),
        (json!(["ru", "st"]), "ru\nst"),
        (json!(["ru\n", "st\n\n"]), "ru\n\nst\n\n"),
        (json!(["", ""]), "\n"),
        (json!([".", "\n."]), ".\n\n."),
        (json!(["", "last\n"]), "\nlast\n"),
    ];
    assert_eq!(cases.len(), 7);
    for (values, expected) in cases {
        let pages: Vec<Value> = values
            .as_array()
            .unwrap()
            .iter()
            .map(|value| json!({"html": value}))
            .collect();
        assert_status(
            &generate("equals", "pages[].html", json!(expected)),
            &json!({"pages": pages}).to_string(),
            true,
            1,
        );
    }
}

#[test]
fn projected_contains_cannot_match_across_removed_separators() {
    let split = json!({"pages": [{"html": "ru"}, {"html": "st"}]}).to_string();
    for (expected, success) in [("rust", false), ("ru\nst", true), ("\n", true)] {
        assert_status(
            &generate("contains", "pages[].html", json!(expected)),
            &split,
            success,
            1,
        );
    }
    assert_status(
        &generate("contains", "pages[].html", json!("rust")),
        &json!({"pages": [{"html": "rust"}]}).to_string(),
        true,
        1,
    );
    assert_status(
        &generate("is_empty", "pages[].html", json!(null)),
        &json!({"pages": []}).to_string(),
        true,
        0,
    );
}

#[test]
fn jq_join_negative_control_proves_the_false_contains_pass() {
    let script = generate("contains", "pages[].html", json!("rust"));
    let payload = json!({"pages": [{"html": "ru"}, {"html": "st"}]}).to_string();
    let (output, assertions) = execute(&script, &payload);
    assert!(!output.status.success());
    assert_eq!(assertions, 1);
    assert!(String::from_utf8_lossy(&output.stdout).contains("not ok 1 capture"));
    let joined = script
        .replace("jq -r -s", "jq -j -s")
        .lines()
        .filter(|line| !line.contains("%$'\\n'}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(joined, script);
    let (output, assertions) = execute(&joined, &payload);
    assert!(output.status.success());
    assert_eq!(assertions, 1);
    assert!(String::from_utf8_lossy(&output.stdout).contains("ok 1 capture"));
}

#[test]
fn numeric_assertions_reject_missing_null_boolean_and_nonnumeric_values() {
    for kind in [
        "greater_than",
        "greater_than_or_equal",
        "less_than",
        "less_than_or_equal",
    ] {
        let script = generate(kind, "value", json!(0));
        for payload in [
            json!({}),
            json!({"value":null}),
            json!({"value":true}),
            json!({"value":false}),
            json!({"value":"nonsense"}),
        ] {
            assert_status(&script, &payload.to_string(), false, 0);
        }
        let valid = if kind.starts_with("greater") { 0.5 } else { -0.5 };
        assert_status(&script, &json!({"value":valid}).to_string(), true, 0);
        assert_status(&script, &json!({"value":-valid}).to_string(), false, 0);
    }
}

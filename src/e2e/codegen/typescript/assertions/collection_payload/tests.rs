use crate::e2e::collection_payload_test_support as support;
use crate::e2e::fixture::Assertion;
use std::collections::HashMap;

fn rendered(text_projection: bool) -> String {
    let mut output = String::new();
    for assertion in support::assertions() {
        super::super::render_assertion(
            &mut output,
            &assertion,
            "result",
            &support::resolver(text_projection),
            false,
            &HashMap::new(),
            "wasm",
            false,
            false,
            true,
        );
    }
    output
}

#[test]
fn wasm_structural_collection_payload_preserves_indices_and_wire_names() {
    let output = rendered(false);
    assert_eq!(output.matches("expect(").count(), 3, "{output}");
    assert!(!output.contains("skipped:"), "{output}");
    assert!(output.contains("[0]"), "{output}");
    assert!(output.contains("[1]"), "{output}");
    assert!(output.contains("\"value\" in value"), "{output}");
    assert!(!output.contains(".parts"), "{output}");
    assert_eq!(support::wire_value()["content"].as_array().unwrap().len(), 2);
}

#[test]
fn wasm_configured_text_projection_refuses_all_structural_assertions() {
    let output = rendered(true);
    assert_eq!(output.matches("skipped:").count(), 3, "{output}");
    assert_eq!(output.matches("untagged_union_text_types text projection").count(), 3);
    assert!(!output.contains("expect("), "{output}");
    assert!(!output.contains(".length"), "{output}");
}

#[test]
fn wasm_collection_payload_executes_serialized_producer_values() {
    let directory = tempfile::tempdir().unwrap();
    let Some((node, tsc)) = native_tools(directory.path()) else {
        return;
    };
    let assertions = rendered(false);
    let old = assertions.replace("\"value\"", "\"text\"");
    assert_ne!(old, assertions);
    let mut count_assertion = String::new();
    assert!(super::render(
        &mut count_assertion,
        &support::assertions()[0],
        "result",
        &support::resolver(false)
    ));
    let source = crate::e2e::template_env::render(
        "typescript/collection_payload_probe.jinja",
        crate::alef_context! { assertions => assertions, old_assertions => old,
        producer => support::wire_value().to_string(), count_assertion => count_assertion },
    );
    let output = run_probe(directory.path(), &node, &tsc, &source);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "verified 7 WASM wire cases, 5 count cases and old canonical-field failure"
    );
}

fn run_probe(
    directory: &std::path::Path,
    node: &std::path::Path,
    tsc: &std::path::Path,
    source: &str,
) -> std::process::Output {
    let path = directory.join("probe.ts");
    std::fs::write(&path, source).unwrap();
    let compiled = std::process::Command::new(tsc)
        .current_dir(directory)
        .args(["--strict", "--target", "ES2022", "--module", "commonjs"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    std::process::Command::new(node)
        .current_dir(directory)
        .arg(directory.join("probe.js"))
        .output()
        .unwrap()
}

fn native_tools(directory: &std::path::Path) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let Ok(node) = which::which_in("node", std::env::var_os("PATH"), directory) else {
        assert!(std::env::var_os("ALEF_REQUIRE_NODE").is_none(), "Node required");
        return None;
    };
    let Ok(tsc) = which::which_in("tsc", std::env::var_os("PATH"), directory) else {
        assert!(std::env::var_os("ALEF_REQUIRE_TSC").is_none(), "tsc required");
        return None;
    };
    Some((node, tsc))
}

#[test]
fn wasm_nested_collection_count_executes_with_strict_unknown_type() {
    let directory = tempfile::tempdir().unwrap();
    let Some((node, tsc)) = native_tools(directory.path()) else {
        return;
    };
    let mut assertions = String::new();
    super::super::render_assertion(
        &mut assertions,
        &Assertion {
            assertion_type: "count_equals".into(),
            field: Some("content.parts[0].items".into()),
            value: Some(serde_json::json!(2)),
            ..Default::default()
        },
        "result",
        &support::resolver(false),
        false,
        &HashMap::new(),
        "wasm",
        false,
        false,
        true,
    );
    assert_eq!(assertions.matches("expect(").count(), 1, "{assertions}");
    assert!(!assertions.contains("skipped:"), "{assertions}");
    let source = crate::e2e::template_env::render(
        "typescript/nested_collection_probe.jinja",
        crate::alef_context! { assertions => assertions, producer => support::nested_wire_value().to_string() },
    );
    let output = run_probe(directory.path(), &node, &tsc, &source);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "verified 4 nested collection count cases"
    );
}

use super::test_case::render_test_case;
use crate::e2e::collection_payload_test_support as support;
use crate::e2e::config::{CallConfig, E2eConfig};
use crate::e2e::fixture::Fixture;
use std::collections::{BTreeSet, HashMap};

fn full_test(assertions: Vec<crate::e2e::fixture::Assertion>) -> String {
    let api = support::api();
    assert_eq!(support::wire_value()["content"].as_array().unwrap().len(), 2);
    let fixture = Fixture {
        id: "parts_reply".into(),
        assertions,
        ..Default::default()
    };
    let e2e = E2eConfig {
        call: CallConfig {
            function: "reply".into(),
            result_var: "result".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let config = crate::core::config::ResolvedCrateConfig {
        untagged_union_text_types: vec!["Body".into()],
        ..Default::default()
    };
    let mut output = String::new();
    render_test_case(
        &mut output,
        &fixture,
        None,
        None,
        &e2e,
        "wasm",
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &api.types,
        &api.enums,
        &api.functions,
        "",
        &config,
        &mut BTreeSet::new(),
        &[],
    );
    output
}

#[test]
fn configured_wasm_text_projection_is_wired_through_full_test_case() {
    let output = full_test(support::assertions());
    assert_eq!(
        output
            .lines()
            .filter(|line| line.trim_start().starts_with("// skipped:"))
            .filter(|line| line.contains("untagged_union_text_types text projection"))
            .count(),
        3,
        "{output}"
    );
    assert!(!output.contains(".length"), "{output}");
    assert!(!output.contains("expect(result"), "{output}");
    assert!(output.contains("it.skip("), "{output}");
    assert!(
        output.contains("parts_reply:  (alef rendered no runnable expectation"),
        "{output}"
    );
    assert!(
        support::resolver(true)
            .text_projected_crossing("content.parts")
            .is_some()
    );
    assert_projected_test_compiles(&output);
}

#[test]
fn wasm_binding_producer_distinguishes_structural_and_configured_text_surfaces() {
    use crate::core::backend::Backend;
    let api = support::api();
    for text in [false, true] {
        let config = crate::core::config::ResolvedCrateConfig {
            name: "sample_core".into(),
            untagged_union_text_types: if text { vec!["Body".into()] } else { Vec::new() },
            ..Default::default()
        };
        let files = crate::backends::wasm::WasmBackend
            .generate_bindings(&api, &config)
            .unwrap();
        let source = files
            .iter()
            .map(|file| file.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if text {
            assert!(source.contains("Option<String>"), "{source}");
            assert!(!source.contains("export type Body ="), "{source}");
        } else {
            assert!(source.contains("serde_wasm_bindgen::to_value"), "{source}");
            assert!(source.contains("JsValue"), "{source}");
        }
    }
}

fn assert_projected_test_compiles(output: &str) {
    let directory = tempfile::tempdir().unwrap();
    let Ok(tsc) = which::which_in("tsc", std::env::var_os("PATH"), directory.path()) else {
        assert!(std::env::var_os("ALEF_REQUIRE_TSC").is_none(), "tsc required");
        return;
    };
    let source = crate::e2e::template_env::render(
        "typescript/projected_test_probe.jinja",
        crate::alef_context! { test => output, expected_checks => usize::from(!output.contains("it.skip(")) },
    );
    let path = directory.path().join("probe.ts");
    std::fs::write(&path, source).unwrap();
    let result = std::process::Command::new(tsc)
        .current_dir(directory.path())
        .args([
            "--strict",
            "--noUnusedLocals",
            "--noUnusedParameters",
            "--module",
            "commonjs",
            "--target",
            "ES2022",
        ])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_node_probe(directory.path());
}

#[test]
fn mixed_projected_fixture_keeps_real_sibling_assertion() {
    let mut assertions = support::assertions();
    assertions.push(crate::e2e::fixture::Assertion {
        assertion_type: "equals".into(),
        field: Some("summary".into()),
        value: Some(serde_json::json!("report")),
        ..Default::default()
    });
    let output = full_test(assertions);
    assert_eq!(
        output
            .lines()
            .filter(|line| line.trim_start().starts_with("// skipped:"))
            .filter(|line| line.contains("untagged_union_text_types text projection"))
            .count(),
        3,
        "{output}"
    );
    assert!(!output.contains("it.skip("), "{output}");
    assert!(output.contains("expect(result.summary)"), "{output}");
    assert!(
        output.contains("console.warn(\"alef missing assertion coverage"),
        "{output}"
    );
    assert_projected_test_compiles(&output);
}

fn assert_node_probe(directory: &std::path::Path) {
    let Ok(node) = which::which_in("node", std::env::var_os("PATH"), directory) else {
        assert!(std::env::var_os("ALEF_REQUIRE_NODE").is_none(), "Node required");
        return;
    };
    let runtime = std::process::Command::new(node)
        .current_dir(directory)
        .arg(directory.join("probe.js"))
        .output()
        .unwrap();
    assert!(runtime.status.success(), "{}", String::from_utf8_lossy(&runtime.stderr));
}

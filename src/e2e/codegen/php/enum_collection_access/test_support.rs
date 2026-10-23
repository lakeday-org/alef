use std::collections::{HashMap, HashSet};

use crate::core::ir::ApiSurface;
use crate::e2e::field_access::FieldResolver;
use crate::e2e::fixture::Assertion;

pub(crate) fn surface() -> ApiSurface {
    let directory = tempfile::tempdir().expect("fixture directory");
    let source = directory.path().join("lib.rs");
    std::fs::write(&source, include_str!("fixture.rs")).expect("write fixture");
    crate::extract::extractor::extract(&[source.as_path()], "sample", "0.0.0", None).expect("extract fixture")
}

pub(crate) fn resolver(surface: &ApiSurface, language: &str) -> FieldResolver {
    let optional = HashSet::from(["choices[0].message.content".to_string()]);
    let arrays = HashSet::from(["choices".to_string()]);
    FieldResolver::new(&HashMap::new(), &optional, &HashSet::new(), &arrays, &HashSet::new())
        .with_ir_enum_map(
            FieldResolver::ir_enum_fields(&surface.types, &surface.enums),
            Some("Reply".to_string()),
        )
        .with_ir_result_fields(
            FieldResolver::ir_result_field_facts(&surface.types, language),
            Some("Reply".to_string()),
        )
}

pub(crate) fn assertions() -> Vec<Assertion> {
    [
        ("count_equals", "parts", serde_json::json!(2)),
        ("equals", "parts[0].text", serde_json::json!("hello")),
        ("equals", "parts[1].text", serde_json::json!(" world")),
    ]
    .into_iter()
    .map(|(kind, suffix, value)| Assertion {
        assertion_type: kind.to_string(),
        field: Some(format!("choices[0].message.content.{suffix}")),
        value: Some(value),
        ..Assertion::default()
    })
    .collect()
}

pub(crate) fn verify_native(language: &str, extension: &str, assertions: &str) {
    verify_native_payload(language, extension, assertions, cases());
}

pub(crate) fn verify_native_payload(
    language: &str,
    extension: &str,
    assertions: &str,
    cases: [(serde_json::Value, &str); 6],
) {
    let directory = tempfile::tempdir().expect("native probe directory");
    let path = std::env::var_os("PATH");
    let executable = match which::which_in(language, path, directory.path()) {
        Ok(executable) => executable,
        Err(error) => {
            assert!(
                language != "ruby" && std::env::var_os(format!("ALEF_REQUIRE_{}", language.to_uppercase())).is_none(),
                "required {language}: {error}"
            );
            tracing::info!("skipping optional {language} probe: {error}");
            return;
        }
    };
    if language == "ruby" {
        assert!(
            ruby_matchers_available(&executable, directory.path()),
            "native proof requires Ruby RSpec matchers"
        );
    }
    let template = format!("{language}/enum_collection_native_probe.{extension}.jinja");
    let source = crate::e2e::template_env::render(&template, crate::alef_context! { assertions => assertions });
    let file = directory.path().join(format!("probe.{extension}"));
    std::fs::write(&file, source).expect("write native probe");
    verify_cases(language, &executable, &file, directory.path(), cases);
}

fn cases() -> [(serde_json::Value, &'static str); 6] {
    [
        (
            serde_json::json!([{"type":"text","text":"hello"},{"type":"text","text":" world"}]),
            "PASS:3",
        ),
        (
            serde_json::json!([{"type":"text","text":"hello"}]),
            "ASSERTION_FAILURE:1:",
        ),
        (
            serde_json::json!([{"type":"text","text":"bad"},{"type":"text","text":" world"}]),
            "ASSERTION_FAILURE:2:",
        ),
        (
            serde_json::json!([{"type":"text","text":"hello"},{"type":"text","text":"bad"}]),
            "ASSERTION_FAILURE:3:",
        ),
        (serde_json::json!("ab"), "expected enum collection payload"),
        (
            serde_json::json!({"first":1,"second":2}),
            "expected enum collection payload",
        ),
    ]
}

fn verify_cases(
    language: &str,
    executable: &std::path::Path,
    file: &std::path::Path,
    directory: &std::path::Path,
    cases: [(serde_json::Value, &str); 6],
) {
    let mut checked = 0;
    for (index, (payload, marker)) in cases.into_iter().enumerate() {
        let output = std::process::Command::new(executable)
            .arg(file)
            .arg(payload.to_string())
            .current_dir(directory)
            .output()
            .expect("execute native probe");
        let evidence = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.status.success(),
            index == 0,
            "{language} case {index}: {evidence}"
        );
        assert!(
            evidence.contains(marker),
            "{language} case {index} expected {marker}: {evidence}"
        );
        checked += 1;
    }
    assert_eq!(checked, 6);
    tracing::info!("{language}: {checked} native cases verified (3 assertions, 5 negative controls)");
}

fn ruby_matchers_available(executable: &std::path::Path, directory: &std::path::Path) -> bool {
    std::process::Command::new(executable)
        .args(["-e", "require 'rspec/expectations'"])
        .current_dir(directory)
        .output()
        .expect("check Ruby RSpec availability")
        .status
        .success()
}

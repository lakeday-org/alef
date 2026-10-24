#![allow(clippy::print_stderr)]

use crate::core::config::ResolvedCrateConfig;
use crate::core::ir::{FieldDef, FunctionDef, TypeDef, TypeRef};
use crate::e2e::codegen::{all_generators, take_skip_records};
use crate::e2e::config::{CallConfig, E2eConfig};
use crate::e2e::fixture::{Assertion, AssertionSkip, Fixture, FixtureGroup};

const LANGUAGES: &[&str] = &[
    "python",
    "go",
    "rust",
    "elixir",
    "r",
    "dart",
    "c",
    "zig",
    "gleam",
    "csharp",
    "java",
    "kotlin",
    "kotlin_android",
    "php",
    "ruby",
    "swift",
    "node",
    "wasm",
];

fn assertion(field: &str) -> Assertion {
    Assertion {
        assertion_type: if field == "error" { "error" } else { "equals" }.into(),
        field: (field != "error").then(|| field.into()),
        value: if field == "error" {
            None
        } else if field.starts_with("error.") {
            Some(serde_json::json!(429))
        } else {
            Some(serde_json::json!("expected"))
        },
        skip: (field == "unavailable").then_some(AssertionSkip::All(true)),
        ..Default::default()
    }
}

pub(super) fn generated(language: &str, fields: &[&str]) -> String {
    generated_result_shape(language, fields, false)
}

fn generated_result_shape(language: &str, fields: &[&str], serde_result: bool) -> String {
    let groups = [FixtureGroup {
        category: "coverage_probe".into(),
        fixtures: vec![Fixture {
            id: "coverage_probe".into(),
            description: "runtime coverage probe".into(),
            assertions: fields.iter().map(|field| assertion(field)).collect(),
            ..Default::default()
        }],
    }];
    let config = probe_config(language);
    let crate_config = ResolvedCrateConfig {
        name: "sample".into(),
        ..Default::default()
    };
    let (mut types, functions) = probe_surface();
    types[0].has_serde = serde_result;
    let _ = take_skip_records();
    let files = all_generators()
        .into_iter()
        .find(|generator| generator.language_name() == language)
        .expect("requested generator exists")
        .generate_gated(&groups, &config, &crate_config, &types, &[], &functions, &[])
        .unwrap_or_else(|error| panic!("{language} generation failed: {error:#}"));
    assert!(!files.is_empty(), "{language} emitted zero files");
    write_proof_artifacts(&files, if serde_result { "zig-json" } else { language }, fields);
    files
        .into_iter()
        .map(|file| file.content)
        .collect::<Vec<_>>()
        .join("\n")
}

fn probe_config(language: &str) -> E2eConfig {
    E2eConfig {
        call: CallConfig {
            function: "probe".into(),
            module: "sample".into(),
            result_var: "result".into(),
            returns_result: true,
            ..Default::default()
        },
        fields_c_types: if language == "c" {
            std::collections::HashMap::from([("probe_reply.unavailable".into(), "skip".into())])
        } else {
            std::collections::HashMap::new()
        },
        result_fields: std::collections::HashSet::from(["content".into()]),
        ..Default::default()
    }
}

fn probe_surface() -> (Vec<TypeDef>, Vec<FunctionDef>) {
    let types = vec![TypeDef {
        name: "ProbeReply".into(),
        rust_path: "sample::ProbeReply".into(),
        is_return_type: true,
        fields: vec![FieldDef {
            name: "content".into(),
            ty: TypeRef::String,
            ..Default::default()
        }],
        ..Default::default()
    }];
    let functions = vec![FunctionDef {
        name: "probe".into(),
        rust_path: "sample::probe".into(),
        return_type: TypeRef::Named("ProbeReply".into()),
        error_type: Some("ProbeError".into()),
        ..Default::default()
    }];
    (types, functions)
}

fn write_proof_artifacts(files: &[crate::core::backend::GeneratedFile], language: &str, fields: &[&str]) {
    if let Some(directory) = std::env::var_os("ALEF_RUNTIME_DIAGNOSTIC_PROOF_DIR") {
        let directory = std::path::PathBuf::from(directory)
            .join(language)
            .join(fields.join("-"));
        assert!(directory.is_absolute(), "proof directory must be absolute");
        for file in files {
            let path = directory.join(&file.path);
            std::fs::create_dir_all(path.parent().expect("generated file has parent")).expect("create proof directory");
            std::fs::write(path, &file.content).expect("write generated proof artifact");
        }
    }
}

#[test]
fn full_generation_preserves_runtime_coverage_diagnostics() {
    let scenarios: &[(&[&str], bool)] = &[
        (&["content"], false),
        (&["content", "unavailable"], true),
        (&["unavailable"], true),
        (&["error", "unavailable"], true),
    ];
    let mut checked = 0;
    let mut failures = Vec::new();
    for language in LANGUAGES {
        for (fields, expect_missing) in scenarios {
            match std::panic::catch_unwind(|| generated(language, fields)) {
                Ok(output) => {
                    checked += 1;
                    if *expect_missing
                        && !output.contains("missing assertion coverage")
                        && !output.contains("no runnable expectation")
                    {
                        failures.push(format!("{language}/{fields:?}: coverage diagnostic missing:\n{output}"));
                    }
                    if !expect_missing && output.contains("missing assertion coverage") {
                        failures.push(format!("{language}: positive control warned"));
                    }
                }
                Err(error) => failures.push(format!("{language}/{fields:?}: generation panicked: {error:?}")),
            }
        }
    }
    eprintln!("runtime diagnostics generation: expected 72 cases, actual {checked}");
    assert_eq!(checked, LANGUAGES.len() * scenarios.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn zig_json_result_preserves_skipped_assertion_diagnostics() {
    let scenarios: &[&[&str]] = &[&["unavailable"], &["content", "unavailable"]];
    for fields in scenarios {
        let output = generated_result_shape("zig", fields, true);
        assert!(output.contains("missing assertion coverage"), "{fields:?}: {output}");
        assert!(output.contains("field 'unavailable' not available"));
        assert!(output.contains("const _result_json = try sample.probe();"));
        assert!(output.contains("defer std.heap.c_allocator.free(_result_json);"));
        assert_eq!(output.contains("var _parsed"), fields.len() > 1);
        assert_eq!(output.contains("const result = &_parsed.value;"), fields.len() > 1);
    }
}

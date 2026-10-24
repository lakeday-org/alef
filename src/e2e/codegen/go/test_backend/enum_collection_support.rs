use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::core::ir::ApiSurface;
use crate::e2e::field_access::FieldResolver;
use crate::e2e::fixture::Assertion;

pub(crate) fn core_source(wire: bool) -> String {
    crate::e2e::template_env::render(
        "go/enum_collection_core.rs.jinja",
        crate::alef_context! {
            wire_key => wire.then_some("\"wireText\""),
        },
    )
}

pub(crate) fn surface(wire: bool) -> ApiSurface {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("lib.rs");
    std::fs::write(&source, core_source(wire)).unwrap();
    crate::extract::extractor::extract(&[source.as_path()], "sample", "1.0.0", None).unwrap()
}

pub(crate) fn resolver(api: &ApiSurface, language: &str) -> FieldResolver {
    let optional = HashSet::from(["choices[0].message.content".to_string()]);
    let arrays = HashSet::from(["choices".to_string()]);
    FieldResolver::new(
        &HashMap::new(),
        &optional,
        &HashSet::from(["choices".to_string()]),
        &arrays,
        &HashSet::new(),
    )
    .with_ir_enum_map(
        FieldResolver::ir_enum_fields(&api.types, &api.enums),
        Some("Reply".to_string()),
    )
    .with_ir_result_fields(
        FieldResolver::ir_result_field_facts(&api.types, language),
        Some("Reply".to_string()),
    )
}

pub(crate) fn assertions() -> Vec<Assertion> {
    crate::e2e::codegen::php::enum_collection_test_support::assertions()
}

pub(crate) fn core_cases(wire: bool) -> Vec<serde_json::Value> {
    let directory = tempfile::tempdir().unwrap();
    let source = crate::e2e::template_env::render(
        "go/enum_collection_core_probe.rs.jinja",
        crate::alef_context! { core => core_source(wire) },
    );
    let path = directory.path().join("probe.rs");
    let binary = directory.path().join(if cfg!(windows) { "probe.exe" } else { "probe" });
    std::fs::write(&path, source).unwrap();
    let compiler = tool("rustc", directory.path());
    let mut command = std::process::Command::new(compiler);
    command
        .arg("--edition=2024")
        .arg(path)
        .arg("-o")
        .arg(&binary)
        .current_dir(directory.path());
    for name in ["serde", "serde_json"] {
        let library = library(name);
        command.arg("--extern").arg(format!("{name}={}", library.display()));
        command
            .arg("-L")
            .arg(format!("dependency={}", library.parent().unwrap().display()));
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let output = std::process::Command::new(binary)
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let cases: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(cases.len(), 6);
    cases
}

pub(crate) fn tool(name: &str, directory: &Path) -> PathBuf {
    which::which_in(name, std::env::var_os("PATH"), directory)
        .unwrap_or_else(|error| panic!("native proof requires {name}: {error}"))
}

fn library(name: &str) -> PathBuf {
    let executable = std::env::current_exe().unwrap();
    std::fs::read_dir(executable.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_str()
                .is_some_and(|file| file.starts_with(&format!("lib{name}-")) && file.ends_with(".rlib"))
        })
        .max_by_key(|path| path.metadata().unwrap().modified().unwrap())
        .unwrap()
}

pub(crate) fn verify_cases(
    cases: Vec<serde_json::Value>,
    markers: [&str; 6],
    mut run: impl FnMut(&serde_json::Value) -> std::process::Output,
) {
    assert_eq!(cases.len(), 6);
    let mut checked = 0;
    for (index, (case, marker)) in cases.iter().zip(markers).enumerate() {
        let output = run(case);
        let evidence = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.success(), index == 0, "case {index}: {evidence}");
        assert!(evidence.contains(marker), "case {index} expected {marker}: {evidence}");
        checked += 1;
    }
    assert_eq!(checked, 6);
}

pub(crate) fn call_surface(wire: bool) -> ApiSurface {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lib.rs");
    std::fs::write(
        &path,
        core_source(wire) + "\npub fn fetch_reply() -> Reply { unimplemented!() }\n",
    )
    .unwrap();
    let api = crate::extract::extractor::extract(&[path.as_path()], "sample", "1.0.0", None).unwrap();
    assert_eq!(api.functions.len(), 1);
    api
}

pub(crate) fn generate_collection_call(
    generator: &dyn crate::e2e::codegen::E2eCodegen,
    api: &ApiSurface,
) -> Vec<crate::core::backend::GeneratedFile> {
    use crate::e2e::fixture::{Fixture, FixtureGroup};
    let e2e = crate::e2e::config::E2eConfig {
        call: crate::e2e::config::CallConfig {
            function: "fetch_reply".to_string(),
            module: "example.com/sample".to_string(),
            returns_result: true,
            overrides: HashMap::from([(
                "c".to_string(),
                crate::e2e::config::CallOverride {
                    function: Some("sample_fetch_reply".to_string()),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        },
        ..Default::default()
    };
    let assertions = assertions();
    assert_eq!(assertions.len(), 3);
    let groups = [FixtureGroup {
        category: "payload".to_string(),
        fixtures: vec![Fixture {
            id: "nested_collection".to_string(),
            assertions,
            ..Default::default()
        }],
    }];
    generator
        .generate(
            &groups,
            &e2e,
            &crate::core::config::ResolvedCrateConfig {
                name: "sample".to_string(),
                ..Default::default()
            },
            &api.types,
            &api.enums,
            &api.functions,
            &api.errors,
        )
        .unwrap()
}

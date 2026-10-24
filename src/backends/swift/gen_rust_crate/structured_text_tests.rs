#![allow(clippy::print_stdout)]

#[path = "structured_numeric_matrix.rs"]
mod numeric_matrix;

use crate::backends::swift::SwiftBackend;
use crate::core::backend::Backend;
use crate::core::config::{NewAlefConfig, ResolvedCrateConfig};
use crate::core::ir::ApiSurface;
use std::path::Path;

const SOURCE: &str = include_str!("structured_text_fixture.rs.txt");

fn fixture(root: &Path) -> (ApiSurface, ResolvedCrateConfig) {
    let source = root.join("src/lib.rs");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, SOURCE).unwrap();
    let api = crate::extract::extractor::extract(&[source.as_path()], "sample_core", "0.0.0", None).unwrap();
    assert_eq!((api.types.len(), api.enums.len()), (2, 3));
    let config: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["swift"]
[[crates]]
name = "sample_core"
sources = ["src/lib.rs"]
untagged_union_text_types = ["BodyValue", "DisplayValue"]
"#,
    )
    .unwrap();
    (api, config.resolve().unwrap().remove(0))
}

#[test]
fn configured_text_getters_preserve_display_and_lossless_typed_reconstruction() {
    let directory = tempfile::tempdir().unwrap();
    let (api, config) = fixture(directory.path());
    let rust = super::emit(&api, &config)
        .unwrap()
        .into_iter()
        .find(|file| file.path.ends_with("src/lib.rs"))
        .unwrap()
        .content;
    assert_eq!(
        rust.matches("fn __alef_json_content(").count(),
        2,
        "extern and implementation only for serde Message: {rust}"
    );
    assert!(
        rust.contains("self.0.content.as_ref().map(|v| v.to_string())"),
        "{rust}"
    );
    assert!(rust.contains("serde_json::to_string(&self.0.content)"), "{rust}");
    let files = SwiftBackend.generate_bindings(&api, &config).unwrap();
    let swift = files
        .iter()
        .filter(|file| file.path.extension().is_some_and(|ext| ext == "swift"))
        .map(|file| file.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(swift.contains("rb.__alef_json_content().toString()"), "{swift}");
    assert!(!swift.contains("rb.content().map"), "{swift}");
    if let Some(path) = std::env::var_os("ALEF_STRUCTURED_TEXT_ARTIFACTS") {
        let root = Path::new(&path);
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(root.join("lib.rs"), SOURCE).unwrap();
        for file in super::emit(&api, &config).unwrap().into_iter().chain(files).chain(
            crate::backends::ffi::FfiBackend
                .generate_bindings(&api, &config)
                .unwrap(),
        ) {
            let path = root.join(&file.path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, file.content).unwrap();
        }
    }
}

fn fixture_assertions() -> Vec<crate::e2e::fixture::Assertion> {
    use crate::e2e::fixture::Assertion;
    [
        ("count_equals", "content.parts", serde_json::json!(3)),
        (
            "equals",
            "content.parts[0].url",
            serde_json::json!("https://example.invalid/image"),
        ),
        ("equals", "content.parts[1].text", serde_json::json!("hello")),
        ("equals", "content.parts[2].text", serde_json::json!(" world")),
        ("count_min", "content.parts", serde_json::json!(2)),
        ("not_empty", "content.parts", serde_json::Value::Null),
        ("contains", "content.parts[1].text", serde_json::json!("ell")),
        ("min_length", "content.parts[1].text", serde_json::json!(5)),
        ("equals", "content.parts[0].active", serde_json::json!(true)),
        ("equals", "content.parts[0].score", serde_json::json!(7)),
        ("greater_than", "content.parts[0].score", serde_json::json!(6)),
        ("greater_than_or_equal", "content.parts[0].score", serde_json::json!(7)),
        (
            "contains_all",
            "content.parts[0].labels",
            serde_json::json!(["one", "two"]),
        ),
        ("is_empty", "content.parts[0].empty", serde_json::Value::Null),
    ]
    .into_iter()
    .map(|(kind, field, value)| Assertion {
        assertion_type: kind.into(),
        field: Some(field.into()),
        values: if kind == "contains_all" {
            value.as_array().cloned()
        } else {
            None
        },
        value: if kind == "contains_all" || value.is_null() {
            None
        } else {
            Some(value)
        },
        ..Default::default()
    })
    .collect()
}

fn scalar_fixture(call: &str, field: &str, value: serde_json::Value) -> crate::e2e::fixture::Fixture {
    crate::e2e::fixture::Fixture {
        id: call.into(),
        call: Some(call.into()),
        assertions: vec![crate::e2e::fixture::Assertion {
            assertion_type: "equals".into(),
            field: Some(field.into()),
            value: Some(value),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn fixture_call(name: &str) -> crate::e2e::config::CallConfig {
    crate::e2e::config::CallConfig {
        function: name.into(),
        ..Default::default()
    }
}

fn generated_suite(api: &ApiSurface, config: &ResolvedCrateConfig) -> String {
    use crate::e2e::codegen::{E2eCodegen, swift::SwiftE2eCodegen};
    use crate::e2e::config::E2eConfig;
    use crate::e2e::fixture::{Fixture, FixtureGroup};
    let assertions = fixture_assertions();
    let fixtures = vec![FixtureGroup {
        category: "structured".into(),
        fixtures: vec![
            Fixture {
                id: "preserved_parts".into(),
                assertions,
                ..Default::default()
            },
            scalar_fixture("scalar_message", "content.text", serde_json::json!("hello")),
            scalar_fixture("unsigned_message", "content.count", serde_json::json!(u64::MAX)),
        ],
    }];
    let e2e = E2eConfig {
        calls: ["scalar_message", "unsigned_message"]
            .into_iter()
            .map(|name| (name.into(), fixture_call(name)))
            .collect(),
        call: fixture_call("sample_message"),
        ..Default::default()
    };
    SwiftE2eCodegen
        .generate(
            &fixtures,
            &e2e,
            config,
            &api.types,
            &api.enums,
            &api.functions,
            &api.errors,
        )
        .unwrap()
        .into_iter()
        .filter(|file| file.path.to_string_lossy().ends_with("StructuredTests.swift"))
        .map(|file| file.content)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn full_swift_dispatch_uses_lossless_capability_for_typed_and_bridge_receivers() {
    let directory = tempfile::tempdir().unwrap();
    let (mut api, config) = fixture(directory.path());
    for opaque in [false, true] {
        api.types
            .iter_mut()
            .find(|definition| definition.name == "Message")
            .unwrap()
            .is_opaque = opaque;
        let suite = generated_suite(&api, &config);
        assert_eq!(suite.matches("JSONSerialization.jsonObject").count(), 16, "{suite}");
        assert!(!suite.contains("skipped:"), "{suite}");
        assert!(!suite.contains(".parts"), "{suite}");
        assert!(suite.contains("numberParts(\"18446744073709551615\")"), "{suite}");
        assert!(suite.contains(r#"as? String), "hello""#), "{suite}");
        if opaque {
            assert!(suite.contains("result.__alef_json_content().toString()"), "{suite}");
        } else {
            assert!(suite.contains("JSONEncoder().encode(result.content)"), "{suite}");
        }
        if let Some(path) = std::env::var_os("ALEF_STRUCTURED_TEXT_ARTIFACTS") {
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(
                Path::new(&path).join(if opaque {
                    "OpaqueTests.swift"
                } else {
                    "TypedTests.swift"
                }),
                suite,
            )
            .unwrap();
        }
    }
}

fn write(root: &Path, name: &str, content: &str) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn native_target(root: &Path) -> std::path::PathBuf {
    match std::env::var_os("ALEF_STRUCTURED_TEXT_NATIVE_TARGET") {
        Some(value) => {
            let target = std::path::PathBuf::from(value);
            assert!(
                target.is_absolute() && target.is_dir(),
                "native cache override must be an existing absolute directory"
            );
            let link = root.join("native-target");
            if !link.exists() {
                #[cfg(unix)]
                std::os::unix::fs::symlink(&target, &link).unwrap();
                #[cfg(not(unix))]
                panic!("native cache override requires Unix symlinks");
            }
            assert_eq!(link.canonicalize().unwrap(), target.canonicalize().unwrap());
            target
        }
        None => root.join("native-target"),
    }
}

fn native_task(root: &Path, name: &str, case: &str, materialize: bool) -> crate::snippets::validators::CapturedStreams {
    let executable = which::which_in("task", std::env::var_os("PATH"), root).unwrap();
    let mut command = std::process::Command::new(executable);
    command
        .arg(name)
        .current_dir(root)
        .env("CARGO_TARGET_DIR", native_target(root))
        .env("RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", "")
        .env("ALEF_STRUCTURED_TEXT_CASE", case)
        .arg(format!("TEST_BINARY={}", std::env::current_exe().unwrap().display()));
    if materialize {
        command.env("ALEF_STRUCTURED_TEXT_MATERIALIZE", root);
    }
    crate::snippets::validators::run_command_streams(&mut command, 600).unwrap()
}

fn prepare_native(root: &Path, api: &ApiSurface, config: &ResolvedCrateConfig) {
    write(root, "src/lib.rs", SOURCE);
    for file in super::emit(api, config)
        .unwrap()
        .into_iter()
        .chain(SwiftBackend.generate_bindings(api, config).unwrap())
        .chain(crate::backends::ffi::FfiBackend.generate_bindings(api, config).unwrap())
        .chain(crate::scaffold::languages::scaffold_ffi(api, config).unwrap())
    {
        write(root, file.path.to_str().unwrap(), &file.content);
    }
    write(
        root,
        "Cargo.toml",
        include_str!("../../../../tests/fixtures/swift_structured_text/core_Cargo.toml"),
    );
    write(
        root,
        "Taskfile.yml",
        include_str!("../../../../tests/fixtures/swift_structured_text/Taskfile.yml"),
    );
    write(
        root,
        "Package.swift",
        include_str!("../../../../tests/fixtures/swift_structured_text/Package.swift"),
    );
    write(
        root,
        "probe/main.swift",
        include_str!("../../../../tests/fixtures/swift_structured_text/main.swift"),
    );
    write(root, "tests/StructuredTests.swift", &generated_suite(api, config));
    write(
        root,
        "packages/swift/Sources/RustBridgeC/module.modulemap",
        include_str!("../../../../tests/fixtures/swift_structured_text/module.modulemap"),
    );
}

#[test]
#[ignore = "requires SwiftPM, swiftc and compiling the actual Rust bridge"]
fn native_structured_text_bridge_preserves_parts() {
    if materialize_if_requested() {
        return;
    }
    let root = tempfile::Builder::new()
        .prefix("alef-691-native-")
        .tempdir()
        .unwrap()
        .keep();
    let (api, config) = fixture(&root);
    prepare_native(&root, &api, &config);
    for task in ["rust", "materialize", "swift", "suite"] {
        let output = native_task(&root, task, "", task == "materialize");
        assert!(output.success, "{root:?} {task}: {}\n{}", output.stdout, output.stderr);
        if task == "suite" {
            assert!(output.stdout.contains("Executed 3 tests"), "{}", output.stdout);
        }
        if task == "swift" {
            assert!(
                output.stdout.contains("structured_text_native_cases=8"),
                "{}",
                output.stdout
            );
        }
    }
    verify_producer_negatives(&root);
    let failure = native_task(&root, "swift", "serialize", false);
    assert!(
        failure.success,
        "serializer failure aborted or escaped: {}\n{}",
        failure.stdout, failure.stderr
    );
    assert!(
        failure.stdout.contains("structured_text_serialization_error_cases=2"),
        "{}",
        failure.stdout
    );
    verify_old_dto_negative(&root);
    numeric_matrix::verify(&root, &api, &config);
    println!(
        "structured_text_xctest_cases=12 assertions_per_suite=16 native_cases=8 serialization_error_cases=2 old_code_negative=1 artifacts={}",
        root.display()
    );
}

fn verify_producer_negatives(root: &Path) {
    for case in [
        "count",
        "first",
        "second",
        "scalar",
        "absent",
        "active",
        "score",
        "labels",
        "empty",
        "scalar_text",
        "unsigned",
    ] {
        let output = native_task(root, "suite-replay", case, false);
        assert!(!output.success, "negative {case} passed: {}", output.stdout);
        assert!(
            output.stdout.contains("failed"),
            "negative {case} failed outside XCTest: {}\n{}",
            output.stdout,
            output.stderr
        );
    }
}

fn materialize_if_requested() -> bool {
    if let Some(root) = std::env::var_os("ALEF_STRUCTURED_TEXT_MATERIALIZE") {
        crate::backends::swift::gen_bindings::bridge_artifacts::materialize_from_build_output(
            "sample_core-swift",
            Path::new("packages/swift"),
            Path::new(&root),
        )
        .unwrap();
        return true;
    }
    false
}

fn verify_old_dto_negative(root: &Path) {
    let path = root.join("packages/swift/Sources/SampleCore/SampleCore.swift");
    let fixed = std::fs::read_to_string(&path).unwrap();
    let assignment = fixed
        .lines()
        .find(|line| line.contains("self.content = try JSONDecoder"))
        .unwrap();
    let old = fixed.replace(
        assignment,
        "        self.content = rb.content().map { BodyValue.text(field0: $0.toString()) }",
    );
    std::fs::write(&path, old).unwrap();
    let output = native_task(root, "swift", "", false);
    assert!(
        !output.success,
        "previous DTO reconstruction unexpectedly retained parts"
    );
    assert!(
        output.stderr.contains("structured payload became text"),
        "old code failed for unrelated reason: {}\n{}",
        output.stdout,
        output.stderr
    );
    std::fs::write(path, fixed).unwrap();
    let output = native_task(root, "swift", "", false);
    assert!(output.success, "restored bridge: {}\n{}", output.stdout, output.stderr);
}

#[test]
fn lossless_companion_requires_parent_and_enum_serde_and_visible_field() {
    let directory = tempfile::tempdir().unwrap();
    let (api, config) = fixture(directory.path());
    for parent_serde in [false, true] {
        for enum_serde in [false, true] {
            for skipped in [false, true] {
                let mut api = api.clone();
                let parent = api.types.iter_mut().find(|item| item.name == "Message").unwrap();
                parent.has_serde = parent_serde;
                parent.fields[0].serde_skip = skipped;
                api.enums
                    .iter_mut()
                    .find(|item| item.name == "BodyValue")
                    .unwrap()
                    .has_serde = enum_serde;
                let rust = super::emit(&api, &config)
                    .unwrap()
                    .into_iter()
                    .find(|file| file.path.ends_with("src/lib.rs"))
                    .unwrap()
                    .content;
                assert_eq!(
                    rust.matches("fn __alef_json_content(").count(),
                    if parent_serde && enum_serde && !skipped { 2 } else { 0 },
                    "parent={parent_serde} enum={enum_serde} skipped={skipped}: {rust}"
                );
                assert!(
                    rust.contains("self.0.content.as_ref().map(|v| v.to_string())"),
                    "{rust}"
                );
            }
        }
    }
}

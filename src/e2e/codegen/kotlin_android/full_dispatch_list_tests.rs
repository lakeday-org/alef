#![allow(clippy::print_stdout)]

use crate::core::backend::Backend;
use crate::core::config::{NewAlefConfig, ResolvedCrateConfig};
use crate::core::ir::ApiSurface;
use crate::e2e::codegen::E2eCodegen;
use crate::e2e::config::{CallConfig, E2eConfig};
use crate::e2e::fixture::{Assertion, Fixture, FixtureGroup};
use std::path::Path;

mod scalar_controls;

fn fixture(root: &Path) -> (ApiSurface, ResolvedCrateConfig) {
    let source = root.join("src/lib.rs");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(
        &source,
        include_str!("../../../backends/swift/gen_rust_crate/structured_text_fixture.rs.txt"),
    )
    .unwrap();
    let api = crate::extract::extractor::extract(&[source.as_path()], "sample_core", "0.0.0", None).unwrap();
    let config: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["kotlin_android"]
[[crates]]
name = "sample_core"
sources = ["src/lib.rs"]
untagged_union_text_types = ["BodyValue"]
[crates.kotlin_android]
package = "sample"
"#,
    )
    .unwrap();
    (api, config.resolve().unwrap().remove(0))
}

fn suite(api: &ApiSurface, config: &ResolvedCrateConfig) -> String {
    let assertions = [
        ("count_equals", "content.parts", serde_json::json!(3)),
        ("equals", "content.parts[1].text", serde_json::json!("hello")),
        ("equals", "content.parts[2].text", serde_json::json!(" world")),
    ]
    .into_iter()
    .map(|(kind, field, value)| Assertion {
        assertion_type: kind.into(),
        field: Some(field.into()),
        value: Some(value),
        ..Default::default()
    })
    .collect();
    let groups = [FixtureGroup {
        category: "structured".into(),
        fixtures: vec![Fixture {
            id: "preserved_parts".into(),
            assertions,
            ..Default::default()
        }],
    }];
    let e2e = E2eConfig {
        call: CallConfig {
            function: "sample_message".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    super::KotlinAndroidE2eCodegen
        .generate(
            &groups,
            &e2e,
            config,
            &api.types,
            &api.enums,
            &api.functions,
            &api.errors,
        )
        .unwrap()
        .into_iter()
        .find(|file| file.path.ends_with("StructuredTest.kt"))
        .expect("one full dispatched test file")
        .content
}

fn write_models(root: &Path, api: &ApiSurface, config: &ResolvedCrateConfig) {
    let files = crate::backends::kotlin_android::KotlinAndroidBackend
        .generate_bindings(api, config)
        .unwrap();
    let mut count = 0;
    for file in files {
        let model = api
            .types
            .iter()
            .map(|item| &item.name)
            .chain(api.enums.iter().map(|item| &item.name))
            .any(|name| file.path.ends_with(format!("{name}.kt")));
        if file.path.components().any(|component| component.as_os_str() == "test")
            || file.path.extension().is_none_or(|extension| extension != "kt")
        {
            continue;
        }
        std::fs::write(root.join(file.path.file_name().unwrap()), file.content).unwrap();
        count += usize::from(model);
    }
    assert_eq!(count, 5, "all real generated DTOs and enums must be compiled");
}

#[test]
fn actual_text_projected_android_producer_and_full_dispatch_agree() {
    let directory = tempfile::tempdir().unwrap();
    let (api, config) = fixture(directory.path());
    let output = suite(&api, &config);
    for name in ["BodyValue", "Piece"] {
        assert!(output.contains(&format!("import sample.{name}\n")), "{output}");
    }
    assert_eq!(output.matches("assertEquals(").count(), 3, "{output}");
    assert!(!output.contains("skipped:"), "{output}");
    if let Some(path) = std::env::var_os("ALEF_ANDROID_STRUCTURED_ARTIFACTS") {
        let root = Path::new(&path);
        std::fs::create_dir_all(root).unwrap();
        write_models(root, &api, &config);
        std::fs::write(root.join("StructuredTest.kt"), output).unwrap();
    }
}

fn native_task(root: &Path, target: &str, classpath: &str, case: &str) -> crate::snippets::validators::CapturedStreams {
    let executable = which::which_in("task", std::env::var_os("PATH"), root).unwrap();
    let mut command = std::process::Command::new(executable);
    command
        .arg(target)
        .arg(format!("CLASSPATH={classpath}"))
        .current_dir(root)
        .env("CARGO_TARGET_DIR", root.join("native-target"))
        .env("RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", "")
        .env("ALEF_STRUCTURED_TEXT_CASE", case);
    crate::snippets::validators::run_command_streams(&mut command, 600).unwrap()
}

fn kotlin_classpath(root: &Path) -> String {
    let compiler = which::which_in("kotlinc", std::env::var_os("PATH"), root)
        .unwrap()
        .canonicalize()
        .unwrap();
    let directory = compiler.parent().unwrap();
    let library = [directory.join("../libexec/lib"), directory.join("../lib")]
        .into_iter()
        .find(|path| path.join("kotlin-test.jar").is_file())
        .unwrap();
    let mut paths = std::fs::read_to_string(root.join("classpath.txt"))
        .unwrap()
        .trim()
        .to_string();
    for name in ["kotlin-test.jar", "kotlinx-coroutines-core-jvm.jar"] {
        let path = library.join(name);
        assert!(path.is_file(), "{}", path.display());
        paths.push(if cfg!(windows) { ';' } else { ':' });
        paths.push_str(path.to_str().unwrap());
    }
    paths.trim().into()
}

fn prepare_native(root: &Path, api: &ApiSurface, config: &ResolvedCrateConfig) {
    write_models(root, api, config);
    for file in crate::backends::jni::JniBackend
        .generate_bindings(api, config)
        .unwrap()
        .into_iter()
        .chain(crate::backends::ffi::FfiBackend.generate_bindings(api, config).unwrap())
        .chain(crate::scaffold::languages::scaffold_ffi(api, config).unwrap())
        .chain(crate::scaffold::languages::scaffold_jni(api, config).unwrap())
    {
        let path = root.join(file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.content).unwrap();
    }
    for (name, content) in [
        (
            "Cargo.toml",
            include_str!("../../../../tests/fixtures/android_structured_text/core_Cargo.toml"),
        ),
        (
            "Taskfile.yml",
            include_str!("../../../../tests/fixtures/android_structured_text/Taskfile.yml"),
        ),
        (
            "pom.xml",
            include_str!("../../../../tests/fixtures/android_structured_text/pom.xml"),
        ),
        (
            "Main.kt",
            include_str!("../../../../tests/fixtures/android_structured_text/Main.kt"),
        ),
    ] {
        std::fs::write(root.join(name), content).unwrap();
    }
}

fn missing_import_control(root: &Path, source: &str, classpath: &str) {
    let old = source
        .replace("import sample.BodyValue\n", "")
        .replace("import sample.Piece\n", "");
    assert_ne!(old, source);
    std::fs::write(root.join("StructuredTest.kt"), old).unwrap();
    let negative = native_task(root, "kotlin-compile", classpath, "");
    assert!(!negative.success, "old missing-import artifact unexpectedly compiled");
    assert!(
        negative.stderr.to_lowercase().contains("unresolved reference") && negative.stderr.contains("BodyValue"),
        "{}\n{}",
        negative.stdout,
        negative.stderr
    );
    std::fs::write(root.join("StructuredTest.kt"), source).unwrap();
    let restored = native_task(root, "kotlin", classpath, "");
    assert!(restored.success, "{}\n{}", restored.stdout, restored.stderr);
}

#[test]
#[ignore = "requires kotlinc, Maven, JDK and compiling the actual JNI/FFI producer"]
fn native_full_android_dispatch_preserves_projected_parts() {
    let root = tempfile::Builder::new()
        .prefix("alef-688-android-")
        .tempdir()
        .unwrap()
        .keep();
    let (api, config) = fixture(&root);
    prepare_native(&root, &api, &config);
    let source = suite(&api, &config);
    std::fs::write(root.join("StructuredTest.kt"), &source).unwrap();
    for target in ["rust", "dependencies"] {
        let output = native_task(&root, target, "", "");
        assert!(
            output.success,
            "{root:?} {target}: {}\n{}",
            output.stdout, output.stderr
        );
    }
    let classpath = kotlin_classpath(&root);
    let positive = native_task(&root, "kotlin", &classpath, "");
    assert!(positive.success, "{root:?}: {}\n{}", positive.stdout, positive.stderr);
    assert!(
        positive.stdout.contains("android_full_dispatch_case=valid"),
        "{}",
        positive.stdout
    );
    for case in ["count", "first", "second", "scalar", "absent"] {
        let output = native_task(&root, "kotlin-run", &classpath, case);
        assert!(
            output.success && output.stdout.contains(&format!("android_full_dispatch_case={case}")),
            "{case}: {}\n{}",
            output.stdout,
            output.stderr
        );
    }
    missing_import_control(&root, &source, &classpath);
    scalar_controls::run(&root, &classpath);
    println!(
        "android_full_dispatch_cases=6 old_import_negative=1 artifacts={}",
        root.display()
    );
}

#![allow(clippy::print_stderr)]

use alef::core::backend::Backend;
use alef::core::config::new_config::NewAlefConfig;
use std::process::Command;

fn generated(ty: &str, annotated: bool, middle: bool) -> String {
    let history = if annotated {
        "#[alef(legacy_constructor_fields = [\"url\", \"enabled\"])]"
    } else {
        ""
    };
    let added = format!("pub added_value: {ty},");
    let fields = if middle {
        format!("pub url: String, {added} pub enabled: bool,")
    } else {
        format!("pub url: String, pub enabled: bool, {added}")
    };
    generated_source(&format!(
        "#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]\n{history}\npub struct Options {{ {fields} }}"
    ))
}

fn generated_source(source: &str) -> String {
    generated_source_result(source).unwrap()
}

fn generated_source_result(source: &str) -> anyhow::Result<String> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lib.rs");
    std::fs::write(&path, source).unwrap();
    let api = alef::extract::extractor::extract(&[&path], "fixture", "1.0.0", None).unwrap();
    let raw: NewAlefConfig = toml::from_str(
        "[workspace]\nlanguages = [\"swift\"]\n[[crates]]\nname = \"fixture\"\nsources = [\"src/lib.rs\"]",
    )
    .unwrap();
    let config = raw.resolve().unwrap().remove(0);
    let files = alef::backends::swift::SwiftBackend.generate_bindings(&api, &config)?;
    assert!(!files.is_empty());
    Ok(files
        .into_iter()
        .map(|file| file.content)
        .collect::<Vec<_>>()
        .join("\n"))
}

const CASES: &[(&str, &str, &str)] = &[
    ("String", "String", "\"\""),
    ("bool", "Bool", "false"),
    ("u32", "UInt32", "0"),
    ("f64", "Double", "0"),
    ("Vec<String>", "[String]", "[]"),
    ("Option<String>", "String?", "nil"),
];

#[test]
fn derived_defaults_keep_recorded_swift_calls_at_both_field_positions() {
    for &(rust_ty, swift_ty, value) in CASES {
        for middle in [false, true] {
            let source = generated(rust_ty, true, middle);
            let expected = format!("addedValue: {swift_ty} = {value}");
            assert!(source.contains(&expected), "{rust_ty}, middle={middle}: {source}");
            assert!(source.contains("url: String, "), "recorded string must remain required");
            assert_eq!(source.matches("public init(url:").count(), 2);
        }
    }
}

#[test]
fn unannotated_scalar_initializers_keep_required_arguments() {
    for &(rust_ty, swift_ty, _) in &CASES[..4] {
        let source = generated(rust_ty, false, false);
        assert!(source.contains(&format!("addedValue: {swift_ty}) {{")), "{source}");
        assert_eq!(source.matches("public init(url:").count(), 1);
    }
}

#[test]
fn emitted_swift_initializers_compile_and_retain_the_default_value() {
    let Ok(swiftc) = which::which("swiftc") else {
        eprintln!("skipping Swift initializer runtime probe: swiftc is unavailable");
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let mut probe = String::new();
    for (index, &(rust_ty, swift_ty, value)) in CASES.iter().enumerate() {
        let source = generated(rust_ty, true, index % 2 == 0);
        let owner = source.split_once("public struct Options").unwrap().1;
        let start = owner.find("public init(url:").unwrap();
        let initializer = &owner[start..];
        let end = initializer.find('\n').unwrap();
        assert!(initializer[..end].contains(&format!("addedValue: {swift_ty} = {value}")));
        let end = initializer.find('}').unwrap() + 1;
        probe.push_str(&format!(
            "struct Probe{index} {{ var url: String; var enabled: Bool; var addedValue: {swift_ty}\n{}\n}}\nlet p{index} = Probe{index}(url: \"sample\", enabled: true)\nprecondition(p{index}.addedValue == {value})\nprecondition(p{index}.url == \"sample\" && p{index}.enabled)\n",
            &initializer[..end]
        ));
    }
    let source_path = directory.path().join("main.swift");
    let binary = directory.path().join("probe");
    std::fs::write(&source_path, probe).unwrap();
    let output = Command::new(swiftc)
        .arg(&source_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(Command::new(binary).status().unwrap().success());
}

#[test]
fn recorded_swift_initializer_retains_its_exact_function_reference() {
    let Ok(swiftc) = which::which("swiftc") else {
        eprintln!("skipping Swift function-reference runtime probe: swiftc is unavailable");
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let mut probe = String::new();
    for (index, &(rust_ty, swift_ty, value)) in CASES.iter().enumerate() {
        let source = generated(rust_ty, true, index % 2 == 0);
        let owner = source.split_once("public struct Options").unwrap().1;
        let start = owner.find("public init(url:").unwrap();
        let end = owner.find("private enum CodingKeys:").unwrap();
        probe.push_str(&format!(
            "struct Probe{index} {{ var url: String; var enabled: Bool; var addedValue: {swift_ty}\n{}\n}}\nlet released{index}: (String, Bool) -> Probe{index} = Probe{index}.init(url:enabled:)\nlet p{index} = released{index}(\"sample\", true)\nprecondition(p{index}.addedValue == {value})\nprecondition(p{index}.url == \"sample\" && p{index}.enabled)\n",
            &owner[start..end]
        ));
    }
    let source_path = directory.path().join("main.swift");
    let binary = directory.path().join("probe");
    std::fs::write(&source_path, probe).unwrap();
    let output = Command::new(swiftc)
        .arg(&source_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(Command::new(binary).status().unwrap().success());
}

#[test]
fn recorded_all_default_initializer_preserves_zero_arg_and_extended_calls() {
    let source = generated_source(
        r#"
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[alef(legacy_constructor_fields = ["enabled"])]
pub struct Options { pub enabled: bool, pub added_value: String }
impl Default for Options {
    fn default() -> Self { Self { enabled: true, added_value: String::new() } }
}
"#,
    );
    assert!(source.contains("public init(enabled: Bool = true) {"));
    assert!(source.contains("public init(enabled: Bool = true, addedValue: String = \"\") {"));
    let Ok(swiftc) = which::which("swiftc") else {
        eprintln!("skipping Swift zero-argument overload probe: swiftc is unavailable");
        return;
    };
    let owner = source.split_once("public struct Options").unwrap().1;
    let start = owner.find("public init(enabled:").unwrap();
    let end = owner.find("private enum CodingKeys:").unwrap();
    let probe = format!(
        "struct Options {{ var enabled: Bool; var addedValue: String\n{}\n}}\nlet zero = Options()\nprecondition(zero.enabled && zero.addedValue.isEmpty)\nlet released: (Bool) -> Options = Options.init(enabled:)\nlet old = released(false)\nprecondition(!old.enabled && old.addedValue.isEmpty)\nlet extended: (Bool, String) -> Options = Options.init(enabled:addedValue:)\nlet new = extended(false, \"new\")\nprecondition(!new.enabled && new.addedValue == \"new\")\n",
        &owner[start..end]
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("main.swift");
    let binary = directory.path().join("probe");
    std::fs::write(&path, probe).unwrap();
    let output = Command::new(swiftc).arg(&path).arg("-o").arg(&binary).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(Command::new(binary).status().unwrap().success());
}

#[test]
fn recorded_initializer_rejects_an_extension_without_a_swift_default() {
    for declaration in [
        "#[derive(Clone, Default, serde::Serialize, serde::Deserialize)] pub struct Nested { pub count: u32 }",
        "#[derive(Clone, Default, serde::Serialize, serde::Deserialize)] pub enum Nested { #[default] Cold, Hot }",
    ] {
        let source = format!(
            "{declaration}\n#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]\n#[alef(legacy_constructor_fields = [\"url\"])]\npub struct Options {{ pub url: String, pub added_value: Nested }}"
        );
        let error = generated_source_result(&source).unwrap_err().to_string();
        assert!(error.contains("Options"), "{error}");
        assert!(error.contains("added_value"), "{error}");
        assert!(error.contains("Swift default"), "{error}");
    }
}

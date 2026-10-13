#![allow(clippy::print_stderr)]

use alef::core::backend::Backend;
use alef::core::config::new_config::NewAlefConfig;
use std::process::Command;

fn generated(ty: &str, annotated: bool, middle: bool) -> String {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lib.rs");
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
    std::fs::write(
        &path,
        format!(
            "#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]\n{history}\npub struct Options {{ {fields} }}"
        ),
    )
    .unwrap();
    let api = alef::extract::extractor::extract(&[&path], "fixture", "1.0.0", None).unwrap();
    let raw: NewAlefConfig = toml::from_str(
        "[workspace]\nlanguages = [\"swift\"]\n[[crates]]\nname = \"fixture\"\nsources = [\"src/lib.rs\"]",
    )
    .unwrap();
    let config = raw.resolve().unwrap().remove(0);
    let files = alef::backends::swift::SwiftBackend
        .generate_bindings(&api, &config)
        .unwrap();
    assert!(!files.is_empty());
    files
        .into_iter()
        .map(|file| file.content)
        .collect::<Vec<_>>()
        .join("\n")
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
        }
    }
}

#[test]
fn unannotated_scalar_initializers_keep_required_arguments() {
    for &(rust_ty, swift_ty, _) in &CASES[..4] {
        let source = generated(rust_ty, false, false);
        assert!(source.contains(&format!("addedValue: {swift_ty}) {{")), "{source}");
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

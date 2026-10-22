use alef::codegen::error_gen::{NAPI_ERROR_INFO_MARKER, gen_napi_error_converter};
use alef::core::ir::{ErrorDef, ErrorVariant, FieldDef, MethodDef, TypeRef};
use std::path::PathBuf;
use std::process::Command;

fn error_definition() -> ErrorDef {
    ErrorDef {
        exception_base: None,
        name: "SampleError".to_string(),
        rust_path: "sample_core::SampleError".to_string(),
        original_rust_path: String::new(),
        variants: vec![ErrorVariant {
            name: "Failure".to_string(),
            fields: vec![FieldDef {
                name: "_0".to_string(),
                ty: TypeRef::String,
                ..Default::default()
            }],
            is_tuple: true,
            ..Default::default()
        }],
        doc: String::new(),
        methods: vec![MethodDef {
            name: "error_type".to_string(),
            return_type: TypeRef::String,
            ..Default::default()
        }],
        binding_excluded: false,
        binding_exclusion_reason: None,
        version: Default::default(),
    }
}

fn serde_json_library() -> PathBuf {
    let executable = std::env::current_exe().unwrap();
    std::fs::read_dir(executable.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_str()
                .is_some_and(|name| name.starts_with("libserde_json-") && name.ends_with(".rlib"))
        })
        .max_by_key(|path| path.metadata().unwrap().modified().unwrap())
        .expect("this integration test links serde_json and its rlib exists beside the executable")
}

fn run_converter(converter: &str) -> Vec<String> {
    let root = tempfile::tempdir().unwrap();
    let source = include_str!("fixtures/node_error_json_harness.rs").replace("/* CONVERTER */", converter);
    let source_path = root.path().join("probe.rs");
    let binary = root.path().join(if cfg!(windows) { "probe.exe" } else { "probe" });
    std::fs::write(&source_path, source).unwrap();
    let library = serde_json_library();
    let compiled = Command::new("rustc")
        .arg("--edition=2024")
        .arg("--extern")
        .arg(format!("serde_json={}", library.display()))
        .arg("-L")
        .arg(format!("dependency={}", library.parent().unwrap().display()))
        .arg(&source_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let result = Command::new(&binary).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn generated_node_error_metadata_round_trips_control_characters() {
    let converter = gen_napi_error_converter(&error_definition(), "sample_core");
    let rows = run_converter(&converter);
    let expected = ["plain", "nul\0value", "control\u{1f}value", "line\nquote\"slash\\"];
    assert_eq!(
        rows.len(),
        expected.len(),
        "every generated converter case must execute"
    );
    for (row, expected) in rows.into_iter().zip(expected) {
        let payload = row.strip_prefix(NAPI_ERROR_INFO_MARKER).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(payload).unwrap_or_else(|error| {
            panic!("the generated Rust converter must emit JSON for {expected:?}: {error}; {payload:?}")
        });
        assert_eq!(parsed["errorType"], expected);
        assert_eq!(parsed["kind"], "Failure");
    }
}

#[test]
fn runtime_probe_rejects_the_previous_rust_debug_escape_format() {
    let converter = gen_napi_error_converter(&error_definition(), "sample_core");
    let legacy = converter.replace(
        "serde_json::Value::String(e.error_type().to_string()).to_string()",
        "format!(\"{:?}\", e.error_type())",
    );
    assert_ne!(
        legacy, converter,
        "the negative control must mutate the generated converter"
    );
    let rows = run_converter(&legacy);
    assert_eq!(rows.len(), 4);
    let payload = rows[1].strip_prefix(NAPI_ERROR_INFO_MARKER).unwrap();
    let error = serde_json::from_str::<serde_json::Value>(payload).unwrap_err();
    assert!(error.to_string().contains("invalid escape"), "{error}");
}

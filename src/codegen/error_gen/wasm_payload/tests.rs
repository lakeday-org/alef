use super::*;
use crate::core::ir::{ErrorVariant, FieldDef, MethodDef, PrimitiveType, TypeRef};

fn sample_error() -> ErrorDef {
    ErrorDef {
        exception_base: None,
        name: "SampleError".into(),
        rust_path: "sample_core::SampleError".into(),
        original_rust_path: String::new(),
        variants: vec![
            ErrorVariant {
                name: "Failure".into(),
                fields: [
                    "message",
                    "name",
                    "code",
                    "kind",
                    "errorCode",
                    "statusCode",
                    "stack",
                    "cause",
                    "constructor",
                    "toString",
                    "payload",
                    "obj",
                    "message_value",
                    "details",
                ]
                .into_iter()
                .map(|name| FieldDef {
                    name: name.into(),
                    ty: TypeRef::String,
                    ..Default::default()
                })
                .collect(),
                ..Default::default()
            },
            ErrorVariant {
                name: "Empty".into(),
                is_unit: true,
                ..Default::default()
            },
        ],
        methods: vec![MethodDef {
            name: "status_code".into(),
            return_type: TypeRef::Primitive(PrimitiveType::U16),
            ..Default::default()
        }],
        doc: String::new(),
        binding_excluded: false,
        binding_exclusion_reason: None,
        version: Default::default(),
    }
}

#[test]
fn payload_reserved_properties_preserve_error_identity_and_metadata() {
    let error = sample_error();
    let properties = super::super::wasm_error_properties(&error);
    let variants = variants(&error, "sample_core::SampleError", &properties);
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].1.len(), 14);
    assert_eq!(
        variants[0]
            .1
            .iter()
            .filter(|field| field.expose_flat)
            .map(|field| field.name)
            .collect::<Vec<_>>(),
        ["obj", "message_value", "details"]
    );
    for name in [
        "message",
        "name",
        "stack",
        "cause",
        "code",
        "kind",
        "errorCode",
        "payload",
        "constructor",
        "__proto__",
        "toString",
        "statusCode",
    ] {
        assert!(reserved_property(name, &properties), "{name}");
    }
    assert!(!reserved_property("details", &properties));
}

#[test]
#[ignore = "requires wasm32 target, wasm-bindgen and Node"]
fn wasm_payload_preserves_actual_thrown_error_display_and_payload() {
    let directory = tempfile::tempdir().unwrap();
    for tool in ["task", "wasm-bindgen", "node"] {
        which::which_in(tool, std::env::var_os("PATH"), directory.path()).unwrap();
    }
    let version = std::process::Command::new("wasm-bindgen")
        .arg("--version")
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        "wasm-bindgen 0.2.121",
        "native probe pins the matching crate schema"
    );
    write_harness(directory.path());
    for metadata in [false, true] {
        let mut error = sample_error();
        if metadata {
            error.methods.push(MethodDef {
                name: "payload".into(),
                return_type: TypeRef::Duration,
                ..Default::default()
            });
        }
        verify_generated_error(directory.path(), &error, metadata);
    }
}

fn verify_generated_error(directory: &std::path::Path, error: &ErrorDef, metadata: bool) {
    let generated = super::super::gen_wasm_error_converter(error, "sample_core", &[]);
    let old = generated.replace("    obj.into()", "    if let sample_core::SampleError::Failure { message, .. } = &e {\n        js_sys::Reflect::set(&obj, &\"message\".into(), &message.as_str().into()).unwrap();\n    }\n    obj.into()");
    assert_ne!(generated, old);
    for (converter, succeeds) in [(&old, false), (&generated, true)] {
        let source = crate::codegen::template_env::render(
            "error_gen/wasm_error_payload_probe.rs.jinja",
            crate::alef_context! { converter => converter },
        );
        std::fs::write(directory.join("lib.rs"), source).unwrap();
        let output = std::process::Command::new("task")
            .arg("verify")
            .current_dir(directory)
            .env("RUSTC_WRAPPER", "")
            .env("RUSTC_WORKSPACE_WRAPPER", "")
            .env("ALEF_EXPECT_PAYLOAD_METADATA", if metadata { "1" } else { "0" })
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.success(), succeeds, "{stdout}\n{stderr}");
        if succeeds {
            assert!(
                stdout.contains("verified 2 thrown errors and 14 payload fields"),
                "{stdout}"
            );
        } else {
            assert!(
                stderr.contains("ERR_ASSERTION") && stderr.contains("Display text must survive"),
                "{stderr}"
            );
        }
    }
}

fn write_harness(directory: &std::path::Path) {
    std::fs::write(directory.join("Cargo.toml"), "[package]\nname = \"error-payload-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[lib]\npath = \"lib.rs\"\ncrate-type = [\"cdylib\"]\n[dependencies]\nwasm-bindgen = \"=0.2.121\"\njs-sys = \"=0.3.98\"\nserde-wasm-bindgen = \"0.6\"\n").unwrap();
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| directory.join("target"))
        .join("wasm-payload-probe");
    let taskfile = crate::codegen::template_env::render(
        "error_gen/wasm_error_payload_task.yml.jinja",
        crate::alef_context! { target => target.display().to_string() },
    );
    std::fs::write(directory.join("Taskfile.yml"), taskfile).unwrap();
    std::fs::write(
        directory.join("probe.cjs"),
        include_str!("../../templates/error_gen/wasm_error_payload_probe.cjs"),
    )
    .unwrap();
}

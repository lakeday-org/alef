use super::wasm_error_properties;
use crate::core::ir::{ErrorDef, MethodDef, PrimitiveType, TypeRef};

fn expression(return_type: TypeRef, returns_ref: bool) -> String {
    let error = ErrorDef {
        methods: vec![MethodDef {
            name: "error_type".to_string(),
            return_type,
            returns_ref,
            ..Default::default()
        }],
        exception_base: None,
        name: "SampleError".to_string(),
        rust_path: "sample_core::SampleError".to_string(),
        original_rust_path: String::new(),
        variants: Vec::new(),
        doc: String::new(),
        binding_excluded: false,
        binding_exclusion_reason: None,
        version: Default::default(),
    };
    wasm_error_properties(&error)
        .into_iter()
        .find(|(name, _)| name == "errorType")
        .unwrap()
        .1
}

#[test]
fn wasm_error_type_properties_borrow_text_and_keep_display_conversion_for_other_values() {
    let cases = [
        (TypeRef::String, true, "&str", "self.value.as_str()", "failure"),
        (TypeRef::String, false, "String", "self.value.clone()", "failure"),
        (TypeRef::String, true, "&String", "&self.value", "failure"),
        (TypeRef::Primitive(PrimitiveType::U16), false, "u16", "42", "42"),
    ];
    assert_eq!(cases.len(), 4);
    for (ty, borrowed, return_type, return_expression, expected) in cases {
        let generated = expression(ty, borrowed);
        compile_property(&generated, return_type, return_expression, expected, true);
    }
    compile_property(
        "wasm_bindgen::JsValue::from_str(&e.error_type().to_string())",
        "&str",
        "self.value.as_str()",
        "failure",
        false,
    );
}

fn compile_property(expression: &str, return_type: &str, return_expression: &str, expected: &str, succeeds: bool) {
    let root = tempfile::tempdir().unwrap();
    let compiler = which::which_in("clippy-driver", std::env::var_os("PATH"), root.path()).unwrap();
    let source = crate::codegen::template_env::render(
        "error_gen/wasm_error_property_probe.rs.jinja",
        crate::alef_context! {
            expression => expression,
            return_type => return_type,
            return_expression => return_expression,
            expected => serde_json::to_string(expected).unwrap(),
        },
    );
    let path = root.path().join("probe.rs");
    let binary = root.path().join(if cfg!(windows) { "probe.exe" } else { "probe" });
    std::fs::write(&path, source).unwrap();
    let output = std::process::Command::new(compiler)
        .args(["--edition=2024", "-D", "warnings"])
        .arg(&path)
        .arg("-o")
        .arg(&binary)
        .current_dir(root.path())
        .output()
        .unwrap();
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.success(), succeeds, "{diagnostics}");
    if succeeds {
        assert!(
            std::process::Command::new(binary)
                .current_dir(root.path())
                .status()
                .unwrap()
                .success()
        );
    } else {
        assert!(diagnostics.contains("unnecessary_to_owned"), "{diagnostics}");
    }
}

#[test]
fn extracted_borrowed_error_type_drives_the_wasm_borrowing_expression() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("lib.rs");
    std::fs::write(
        &path,
        r#"
#[derive(Debug, thiserror::Error)]
pub enum SampleError {
    #[error("failure")]
    Failure,
}
impl SampleError {
    pub fn error_type(&self) -> &str { "failure" }
}
"#,
    )
    .unwrap();
    let api = crate::extract::extractor::extract(&[path.as_path()], "sample_core", "1.0.0", None).unwrap();
    assert_eq!(api.errors.len(), 1);
    let error = &api.errors[0];
    let method = error.methods.iter().find(|method| method.name == "error_type").unwrap();
    assert_eq!(method.return_type, TypeRef::String);
    assert!(method.returns_ref);
    let generated = super::gen_wasm_error_converter(error, "sample_core", &[]);
    assert!(
        generated.contains("wasm_bindgen::JsValue::from_str(e.error_type())"),
        "{generated}"
    );
    assert!(!generated.contains("e.error_type().to_string()"), "{generated}");
}

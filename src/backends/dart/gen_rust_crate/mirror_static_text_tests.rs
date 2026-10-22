use crate::core::ir::{CoreWrapper, TypeRef};

fn compile_and_run(generated: &str) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.rs");
    let binary = directory.path().join(format!("probe{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source, generated).unwrap();
    let compile = std::process::Command::new(which::which("rustc").expect("Rust compiler is required"))
        .arg("--edition=2024")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}\n{generated}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = std::process::Command::new(binary).output().unwrap();
    assert!(
        run.status.success(),
        "{}\n{generated}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn borrowed_static_mirror_introspection_returns_errors_without_leaking_or_panicking() {
    for optional in [false, true] {
        for tuple in [false, true] {
            let mut error = super::tests::error_with_text_field(CoreWrapper::StaticStr, optional);
            let variant = &mut error.variants[0];
            variant.is_tuple = tuple;
            if tuple {
                variant.fields[0].name = "_0".into();
                if optional {
                    variant.fields[0].ty = TypeRef::Optional(Box::new(TypeRef::String));
                }
            }
            let mut generated = String::new();
            super::emit_mirror_error(&mut generated, &error, "demo");
            super::super::error_bridge::emit(&mut generated, &[error], "demo");
            assert!(!generated.contains("Box::leak"), "{generated}");
            let generated = generated
                .lines()
                .filter(|line| {
                    let line = line.trim_start();
                    line != "#[frb]" && !line.starts_with("#[frb(")
                })
                .collect::<Vec<_>>()
                .join("\n");
            compile_and_run(&format!("{generated}\n{}", probe(optional, tuple)));
        }
    }
}

fn probe(optional: bool, tuple: bool) -> String {
    let (core_type, native, unknown) = if optional {
        (
            "Option<&'static str>",
            "Some(\"native\")",
            "Some(\"Dart text\".to_string())",
        )
    } else {
        ("&'static str", "\"native\"", "\"Dart text\".to_string()")
    };
    let (declaration, construction, pattern, field) = if tuple {
        (
            format!("Denied({core_type})"),
            format!("demo::ApiError::Denied({native})"),
            "demo::ApiError::Denied(reason)",
            "field0",
        )
    } else {
        (
            format!("Denied {{ reason: {core_type} }}"),
            format!("demo::ApiError::Denied {{ reason: {native} }}"),
            "demo::ApiError::Denied { reason }",
            "reason",
        )
    };
    format!(
        r#"
mod demo {{
    #[derive(Debug)]
    pub enum ApiError {{ {declaration} }}
    impl ApiError {{ pub fn retry_after(&self) -> bool {{ true }} }}
    impl std::fmt::Display for ApiError {{
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ f.write_str("denied") }}
    }}
}}
fn main() {{
    let mirror = ApiError::try_from({construction}).unwrap();
    assert!(mirror.retry_after().unwrap());
    let original = demo::ApiError::try_from(&mirror).unwrap();
    let {pattern} = original;
    assert_eq!(reason, {native});
    let unknown = ApiError::Denied {{ {field}: {unknown} }};
    assert!(unknown.retry_after().unwrap_err().contains("reconstructing borrowed error"));
    assert!(demo::ApiError::try_from(&unknown).is_err());
}}
"#
    )
}

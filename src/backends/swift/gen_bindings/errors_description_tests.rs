use super::*;

fn extracted_error() -> ErrorDef {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lib.rs");
    std::fs::write(
        &path,
        r#"
#[derive(Debug, thiserror::Error)]
pub enum SampleError {
    #[error("not_found: {message}")]
    NotFound { message: String },
    #[error("access_denied: filter/block: {message}")]
    Blocked { message: String },
    #[error("policy_rejected: {url} - {reason}")]
    PolicyRejected { message: String, url: String, reason: String },
    #[error("{{prefix}}: \"{message}\"\\tail\n")]
    Escaped { message: String },
    #[error("empty operation")]
    Empty,
    #[error("{detail}")]
    Synthetic { detail: String },
    #[error("debug: {message:?}")]
    Unsupported { message: String },
    #[error("arbitrary custom display")]
    Opaque,
}
"#,
    )
    .unwrap();
    let api = crate::extract::extractor::extract(&[path.as_path()], "sample_core", "0.0.0", None).unwrap();
    assert_eq!(api.errors.len(), 1);
    let mut error = api.errors.into_iter().next().unwrap();
    assert_eq!(error.variants.len(), 8);
    for variant in &mut error.variants {
        if matches!(variant.name.as_str(), "Synthetic" | "Opaque") {
            variant.message_template = None;
        }
    }
    error
}

#[test]
fn description_templates_reject_unsupported_or_unavailable_interpolation() {
    let mut variant = extracted_error().variants.remove(0);
    let cases = [
        ("prefix: {message}", true),
        ("{{literal}}: {message}", true),
        ("unclosed {message", false),
        ("unexpected }", false),
        ("debug {message:?}", false),
        ("unknown {missing}", false),
        ("nested {message{}}", false),
    ];
    assert_eq!(cases.len(), 7);
    for (template, supported) in cases {
        variant.message_template = Some(template.into());
        assert_eq!(
            template_expression(&variant, &SwiftMapper).is_some(),
            supported,
            "{template}"
        );
    }
    variant.fields[0].binding_excluded = true;
    variant.message_template = Some("prefix: {message}".into());
    assert!(template_expression(&variant, &SwiftMapper).is_none());
}

#[test]
fn descriptions_use_canonical_labels_and_preserve_existing_case_arity() {
    let error = extracted_error();
    let mut declaration = String::new();
    super::super::emit_error(&error, "SampleCore", &mut declaration, &SwiftMapper);
    assert!(declaration.contains("case notFound(message: String)"));
    assert!(declaration.contains("case policyRejected(message: String, url: String, reason: String)"));
    assert!(declaration.contains("case empty\n"));
    assert!(declaration.contains("extension SampleError: Swift.CustomStringConvertible"));
    let policy = description_case(&error.variants[2], &SwiftMapper);
    assert_eq!(policy.pattern, "(message: _, url: let url, reason: let reason)");
    assert!(policy.expression.contains("String(describing: url)"));
    assert!(policy.expression.contains("String(describing: reason)"));
}

fn compile_probe(directory: &std::path::Path, swiftc: &std::path::Path, declaration: &str) -> std::process::Output {
    let source = crate::backends::swift::template_env::render(
        "error_description_probe.swift.jinja",
        crate::alef_context! { declaration => declaration },
    );
    let path = directory.join("main.swift");
    let binary = directory.join("probe");
    std::fs::write(&path, source).unwrap();
    let output = std::process::Command::new(swiftc)
        .current_dir(directory)
        .arg("-warnings-as-errors")
        .arg(&path)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::process::Command::new(binary)
        .current_dir(directory)
        .output()
        .unwrap()
}

#[test]
#[ignore = "requires Swift compiler"]
fn actual_thrown_swift_errors_preserve_descriptions_and_raw_payloads() {
    let directory = tempfile::tempdir().unwrap();
    let swiftc = which::which_in("swiftc", std::env::var_os("PATH"), directory.path())
        .expect("native Swift error proof requires swiftc");
    let mut declaration = String::new();
    super::super::emit_errors(&[extracted_error()], "SampleCore", &mut declaration, &SwiftMapper);
    let old = declaration.replace(
        "extension SampleError: Swift.CustomStringConvertible",
        "extension SampleError",
    );
    assert_ne!(old, declaration);
    let negative = compile_probe(directory.path(), &swiftc, &old);
    assert!(!negative.status.success());
    assert!(String::from_utf8_lossy(&negative.stderr).contains("Display text must survive"));
    let positive = compile_probe(directory.path(), &swiftc, &declaration);
    assert!(
        positive.status.success(),
        "{}",
        String::from_utf8_lossy(&positive.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&positive.stdout).trim(),
        "verified 8 typed descriptions and ordinary error passthrough"
    );
}

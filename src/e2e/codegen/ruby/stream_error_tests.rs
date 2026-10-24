use crate::core::config::ResolvedCrateConfig;
use crate::core::ir::{ErrorDef, ErrorVariant};
use crate::e2e::config::E2eConfig;
use crate::e2e::fixture::Fixture;

fn render(declared: Option<&str>, errors: &[ErrorDef]) -> String {
    let fixture: Fixture = serde_json::from_value(serde_json::json!({
        "id": "stream_error", "input": null, "assertions": [
            {"type": "error", "value": declared},
            {"type": "equals", "field": "error.status_code", "value": 401}
        ]
    }))
    .expect("fixture");
    let mut config = E2eConfig::default();
    config.call.function = "chat_stream".into();
    config.call.streaming = Some(crate::core::config::e2e::StreamingConfig::Enabled(true));
    let _ = crate::e2e::codegen::take_skip_records();
    super::spec_file::render_spec_file(
        "streaming",
        &[&fixture],
        "Sample",
        None,
        "sample",
        None,
        &Default::default(),
        false,
        &config,
        false,
        false,
        &[],
        &ResolvedCrateConfig::default(),
        &[],
        errors,
        &[],
        &[],
    )
}

#[test]
fn streaming_errors_require_runtime_error_even_without_a_declared_value() {
    let source = render(None, &[]);
    assert!(
        source.contains("{ |_chunk| } }.to raise_error(RuntimeError)"),
        "{source}"
    );
    assert!(
        source.contains("has no accessor for error field error.status_code"),
        "{source}"
    );
    let records = crate::e2e::codegen::take_skip_records();
    assert_eq!(records.len(), 1, "{records:?}");
    assert_eq!(records[0].field, "equals");
    let assertion = source.find("}.to raise_error(RuntimeError)").unwrap();
    let marker = source.find("# skipped: assertion type 'equals'").unwrap();
    let warning = source.find("warn 'alef missing assertion coverage").unwrap();
    assert!(assertion < marker && marker < warning, "{source}");
    assert!(source[warning..].contains("\n  end"), "{source}");
    assert_eq!(source.matches("# skipped: assertion type 'equals'").count(), 1);
    if let Ok(proof_root) = std::env::var("ALEF_RUNTIME_DIAGNOSTIC_PROOF_DIR") {
        let path = std::path::Path::new(&proof_root).join("ruby-stream-error");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("fixture_spec.rb"), &source).unwrap();
    }
}

#[test]
fn streaming_literal_error_messages_retain_the_escaped_predicate() {
    let source = render(Some("missing.field[0]"), &[]);
    assert!(source.contains("raise_error(RuntimeError) { |error|"), "{source}");
    assert!(source.contains(r"/missing\.field\[0\]/"), "{source}");
}

#[test]
fn streaming_known_variants_preserve_identity_and_record_only_status_gaps() {
    let errors = vec![ErrorDef {
        exception_base: None,
        name: "ApiError".into(),
        variants: vec![ErrorVariant {
            name: "Authentication".into(),
            ..Default::default()
        }],
        rust_path: "sample::ApiError".into(),
        original_rust_path: String::new(),
        doc: String::new(),
        methods: Vec::new(),
        binding_excluded: false,
        binding_exclusion_reason: None,
        version: Default::default(),
    }];
    let source = render(Some("Authentication"), &errors);
    assert!(source.contains("raise_error(Sample::AuthenticationError)"), "{source}");
    assert!(!source.contains("declared error variant 'Authentication'"), "{source}");
    assert!(!source.contains("/Authentication/"), "{source}");
    let assertion = source.find("raise_error(Sample::AuthenticationError)").unwrap();
    let warning = source.find("warn 'alef missing assertion coverage").unwrap();
    assert!(assertion < warning && source[warning..].contains("\n  end"), "{source}");
    let records = crate::e2e::codegen::take_skip_records();
    assert_eq!(records.len(), 1, "{records:?}");
}

#[test]
fn generated_streaming_specs_omit_rust_only_errors_without_skipping_assertions() {
    let errors = super::error_matcher::tests::errors_with_hidden();
    for (declared, expected) in [
        (None, "be_a(Sample::RootError).or be_a(RuntimeError)"),
        (Some("BadRequest"), "raise_error(Sample::BadRequestError)"),
        (Some("HiddenSyntax"), "/HiddenSyntax/"),
    ] {
        let source = render(declared, &errors);
        assert!(source.contains(expected), "{source}");
        assert!(!source.contains("Sample::Hidden"), "{source}");
        assert!(source.contains("it "), "fixture was not emitted: {source}");
    }
}

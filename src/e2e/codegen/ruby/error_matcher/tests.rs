use super::render_raise_error_clause;
use crate::core::ir::ErrorDef;
use crate::e2e::fixture::Fixture;

pub(super) fn fixture(value: Option<&str>) -> Fixture {
    serde_json::from_value(serde_json::json!({
        "id": "error_matcher", "input": null,
        "assertions": [{"type": "error", "value": value}]
    }))
    .expect("fixture")
}

pub(super) fn errors() -> Vec<ErrorDef> {
    serde_json::from_value(serde_json::json!([
        {"name": "RootError", "rust_path": "sample::RootError", "variants": [], "doc": ""},
        {"name": "ApiError", "exception_base": "RootError", "rust_path": "sample::ApiError", "doc": "",
         "variants": [{"name": "BadRequest", "is_unit": true, "doc": ""}, {"name": "Connection", "is_unit": true, "doc": ""}]}
    ]))
    .expect("error registry")
}

#[test]
fn known_variants_use_the_native_canonical_classes_without_identity_skips() {
    for (variant, expected) in [("BadRequest", "BadRequestError"), ("Connection", "ApiConnectionError")] {
        assert_eq!(
            render_raise_error_clause(&fixture(Some(variant)), &errors(), "Sample"),
            format!("raise_error(Sample::{expected})")
        );
    }
}

#[test]
fn unknown_native_errors_retain_runtime_error_and_literal_message_checks() {
    assert_eq!(
        render_raise_error_clause(&fixture(None), &[], "Sample"),
        "raise_error(RuntimeError)"
    );
    let clause = render_raise_error_clause(&fixture(Some("missing.field[0]")), &[], "Sample");
    assert!(clause.starts_with("raise_error(RuntimeError) { |error|"), "{clause}");
    assert!(clause.contains(r"/missing\.field\[0\]/"), "{clause}");
}

#[test]
fn bare_and_message_checks_accept_only_known_families_or_native_fallback() {
    for value in [None, Some("missing.field[0]")] {
        let clause = render_raise_error_clause(&fixture(value), &errors(), "Sample");
        assert!(clause.starts_with("raise_error(StandardError) { |error|"), "{clause}");
        assert!(
            clause.contains("be_a(Sample::RootError).or be_a(RuntimeError)"),
            "{clause}"
        );
        if value.is_some() {
            assert!(clause.contains(r"/missing\.field\[0\]/"), "{clause}");
        }
    }
}

fn errors_with_excluded() -> Vec<ErrorDef> {
    let mut errors = errors();
    errors.extend(
        serde_json::from_value::<Vec<ErrorDef>>(serde_json::json!([
            {"name": "HiddenError", "rust_path": "sample::HiddenError", "doc": "", "binding_excluded": true,
             "variants": [{"name": "Compile", "is_unit": true, "doc": ""}]}
        ]))
        .expect("excluded error"),
    );
    errors
}

#[test]
fn bare_and_message_checks_never_name_an_error_the_extension_does_not_define() {
    for value in [None, Some("missing.field[0]")] {
        let clause = render_raise_error_clause(&fixture(value), &errors_with_excluded(), "Sample");
        assert!(!clause.contains("HiddenError"), "{clause}");
        assert!(
            clause.contains("be_a(Sample::RootError).or be_a(RuntimeError)"),
            "{clause}"
        );
    }
}

#[test]
fn a_declared_variant_of_an_excluded_error_falls_back_to_the_message_check() {
    let clause = render_raise_error_clause(&fixture(Some("Compile")), &errors_with_excluded(), "Sample");
    assert!(
        !clause.contains("HiddenError") && !clause.contains("CompileError"),
        "{clause}"
    );
    assert!(clause.starts_with("raise_error(StandardError) { |error|"), "{clause}");
}

pub(in crate::e2e::codegen::ruby) fn errors_with_hidden() -> Vec<ErrorDef> {
    let mut errors = errors();
    errors.push(
        serde_json::from_value(serde_json::json!({
            "name": "HiddenCompileError", "rust_path": "sample::HiddenCompileError", "doc": "",
            "binding_excluded": true,
            "variants": [{"name": "HiddenSyntax", "is_unit": true, "doc": ""}]
        }))
        .expect("Rust-only error"),
    );
    errors
}

#[test]
fn rust_only_errors_never_supply_matcher_constants() {
    for (declared, expected) in [
        (None, "be_a(Sample::RootError).or be_a(RuntimeError)"),
        (Some("BadRequest"), "raise_error(Sample::BadRequestError)"),
        (Some("HiddenSyntax"), "/HiddenSyntax/"),
        (Some("missing.field[0]"), r"/missing\.field\[0\]/"),
    ] {
        let clause = render_raise_error_clause(&fixture(declared), &errors_with_hidden(), "Sample");
        assert!(clause.contains(expected), "{clause}");
        assert!(!clause.contains("Sample::Hidden"), "{clause}");
    }
    let hidden = errors_with_hidden().pop().expect("hidden error");
    assert_eq!(
        render_raise_error_clause(&fixture(None), &[hidden], "Sample"),
        "raise_error(RuntimeError)"
    );
}

#[test]
fn full_generator_retains_error_fixtures_and_only_names_exported_classes() {
    use crate::e2e::codegen::{E2eCodegen, ruby::RubyCodegen};
    let mut config = crate::e2e::config::E2eConfig::default();
    config.call.module = "Sample".into();
    config.call.function = "bad_request".into();
    let groups = [crate::e2e::fixture::FixtureGroup {
        category: "errors".into(),
        fixtures: vec![fixture(None)],
    }];
    let files = RubyCodegen
        .generate(
            &groups,
            &config,
            &Default::default(),
            &[],
            &[],
            &[],
            &errors_with_hidden(),
        )
        .expect("Ruby generation");
    let specs: Vec<_> = files
        .iter()
        .filter(|file| file.path.to_string_lossy().ends_with("errors_spec.rb"))
        .collect();
    assert_eq!(specs.len(), 1, "one emitted spec must be checked");
    let source = &specs[0].content;
    assert!(
        source.contains("be_a(Sample::RootError).or be_a(RuntimeError)"),
        "{source}"
    );
    assert!(!source.contains("Sample::HiddenCompileError"), "{source}");
    assert!(source.contains("it "), "fixture must be emitted: {source}");
}

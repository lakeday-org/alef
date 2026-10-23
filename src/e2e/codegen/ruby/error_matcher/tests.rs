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

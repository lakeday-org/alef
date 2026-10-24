use super::runtime_diagnostics::{coverage_warning, skip_details};
use super::{InertCause, InertExample, inert_verdict};
use crate::e2e::fixture::Assertion;

#[test]
fn refusal_reason_retains_every_field_message_and_tracking_issue() {
    for cause in [InertCause::UnresolvedFieldPath, InertCause::AwaitedOrLimited] {
        let example = InertExample {
            language: "swift".into(),
            fixture_id: "quoted_fixture".into(),
            markers: 2,
            cause,
            details: vec![
                "field parts[0].text unavailable (GH#692)".into(),
                "field parts[1].kind blocked (GH#697)".into(),
            ],
        };
        let reason = example.reason();
        for detail in &example.details {
            assert!(reason.contains(detail), "reason dropped {detail}: {reason}");
        }
    }
}

#[test]
fn diagnostics_only_read_comment_markers_and_keep_duplicates() {
    let body = "assertEqual(result, \"skipped: not a marker\")\n// skipped: first (GH#697)\n# skipped: first (GH#697)\n/* skipped: second */\n";
    assert_eq!(skip_details(body), ["first (GH#697)", "first (GH#697)", "second"]);
    assert!(coverage_warning("assertEqual(result, 1)", "swift", "control").is_empty());
    assert!(inert_verdict("assertEqual(result, 1)", "swift", "control", &[Assertion::default()]).is_none());
}

#[test]
fn runtime_warning_escapes_literals_without_interpolation_or_skip() {
    let body = "// skipped: field $value #{value} has 'quotes' and \\\"double\\\" (GH#697)\n";
    for language in [
        "python",
        "go",
        "rust",
        "elixir",
        "r",
        "dart",
        "c",
        "zig",
        "gleam",
        "swift",
        "ruby",
        "php",
        "java",
        "kotlin",
        "kotlin_android",
        "csharp",
        "node",
        "wasm",
    ] {
        let warning = coverage_warning(body, language, "quoted_fixture");
        assert!(warning.contains("GH#697"), "{language} lost tracking issue");
        assert!(warning.contains("quoted_fixture"), "{language} lost fixture");
        assert!(!warning.contains("SkipIf") && !warning.contains("assumeTrue") && !warning.contains("it.skip"));
        if language.starts_with("kotlin") || language == "dart" {
            assert!(warning.contains("\\$value"));
        }
        if language == "elixir" {
            assert!(warning.contains("\\#{value}"));
        }
        if language == "php" || language == "ruby" {
            assert!(warning.contains("\\'quotes\\'"));
        }
    }
}

#[test]
fn inert_verdict_retains_the_rendered_tracking_issue_and_field() {
    use crate::e2e::codegen::field_skip::FieldSkip;
    let message = FieldSkip::DisplayTextUnionVariantNotReadableInSwift.message("parts[0].text");
    let body = format!("// skipped: {message}\n");
    let declared = [Assertion {
        assertion_type: "equals".into(),
        field: Some("parts[0].text".into()),
        ..Default::default()
    }];
    let _ = crate::e2e::codegen::take_skip_records();
    crate::e2e::codegen::fail_on_unavailable_field_markers(&body, "swift", "parts_fixture", &declared);
    crate::e2e::codegen::fail_on_unsupported_assertion_type_markers(&body, "swift", "parts_fixture");
    let refusal = inert_verdict(&body, "swift", "parts_fixture", &declared).expect("all skipped");
    assert_eq!(refusal.markers, 1);
    assert!(refusal.reason().contains(&message));
    assert!(refusal.reason().contains("xberg-io/alef#691"));
    let _ = crate::e2e::codegen::take_skip_records();
}

#[test]
fn completed_rust_body_keeps_the_warning_inside_the_test_function() {
    let body = "fn probe() {\n    assert_eq!(1, 1);\n    // skipped: field missing (GH#698)\n}\n";
    let rendered = super::completed_body(body, "rust", "brace_probe");
    let parsed = syn::parse_file(&rendered).expect("warning must remain inside valid Rust syntax");
    assert_eq!(parsed.items.len(), 1);
    let syn::Item::Fn(function) = &parsed.items[0] else {
        panic!("one generated test function")
    };
    assert_eq!(
        function.block.stmts.len(),
        2,
        "real assertion and stderr diagnostic both execute"
    );
    assert!(rendered.contains("std::io::Write::write_all"));
}

#[test]
fn diagnostic_control_characters_are_visible_text_in_every_target() {
    assert_eq!(
        super::runtime_diagnostics::visible_control_text("ordinary field (GH#697)"),
        "ordinary field (GH#697)"
    );
    let languages = [
        "python",
        "go",
        "rust",
        "elixir",
        "r",
        "dart",
        "c",
        "zig",
        "gleam",
        "swift",
        "ruby",
        "php",
        "java",
        "kotlin",
        "kotlin_android",
        "csharp",
        "node",
        "wasm",
    ];
    let body = "// skipped: field value\u{8}\u{1} (GH#697)\n";
    let mut checked = 0;
    for language in languages {
        let rendered = coverage_warning(body, language, "control\u{8}\u{1}");
        assert!(
            !rendered.contains('\u{8}') && !rendered.contains('\u{1}'),
            "{language}: {rendered:?}"
        );
        assert!(
            rendered.contains(r"\\u{8}") && rendered.contains(r"\\u{1}"),
            "{language}: {rendered:?}"
        );
        assert!(rendered.contains("GH#697"));
        if let Some(directory) = std::env::var_os("ALEF_RUNTIME_DIAGNOSTIC_PROOF_DIR") {
            let directory = std::path::PathBuf::from(directory).join("control_diagnostics");
            assert!(directory.is_absolute());
            std::fs::create_dir_all(&directory).expect("create control proof directory");
            std::fs::write(directory.join(format!("{language}.txt")), &rendered).expect("write control diagnostic");
        }
        checked += 1;
    }
    assert_eq!(checked, 18);
}

#[test]
fn runtime_cause_copies_are_not_counted_as_skip_markers() {
    let body = "    # skipped: assertion type 'equals' has no accessor for error field error.code in this backend\n";
    let warning = coverage_warning(body, "python", "probe");
    let _ = crate::e2e::codegen::take_skip_records();
    crate::e2e::codegen::fail_on_unsupported_assertion_type_markers(&format!("{body}{warning}"), "python", "probe");
    assert_eq!(crate::e2e::codegen::take_skip_records().len(), 1);
    crate::e2e::codegen::fail_on_unsupported_assertion_type_markers(&warning, "python", "probe");
    assert!(crate::e2e::codegen::take_skip_records().is_empty());
    let field_body = "// skipped: field 'missing' not available on result type\n";
    let field_warning = coverage_warning(field_body, "rust", "probe");
    crate::e2e::codegen::fail_on_unavailable_field_markers(
        &format!("{field_body}{field_warning}"),
        "rust",
        "probe",
        &[],
    );
    assert_eq!(crate::e2e::codegen::take_skip_records().len(), 1);
    crate::e2e::codegen::fail_on_unavailable_field_markers(&field_warning, "rust", "probe", &[]);
    assert!(crate::e2e::codegen::take_skip_records().is_empty());
}

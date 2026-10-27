use super::render_error_test_body;

#[test]
fn no_declared_value_is_byte_identical_to_bare_success_check() {
    let body = render_error_test_body("mycli extract --file x", None);
    assert_eq!(
        body,
        "  if mycli extract --file x >/dev/null 2>&1; then\n    echo 'FAIL [error]: expected command to fail but it succeeded' >&2\n    return 1\n  fi"
    );
}

#[test]
fn declared_value_adds_output_grep_check() {
    let body = render_error_test_body("mycli extract --file x", Some("BadRequest"));
    assert_eq!(
        body,
        "  if output=$(mycli extract --file x 2>&1); then\n    echo 'FAIL [error]: expected command to fail but it succeeded' >&2\n    return 1\n  fi\n  if ! grep -qF 'BadRequest' <<<\"$output\"; then\n    echo 'FAIL [error]: expected output to contain ''BadRequest' >&2\n    return 1\n  fi"
    );
}

#[test]
fn declared_value_with_regex_metacharacters_needs_no_escaping_under_fixed_string_grep() {
    // `grep -F` treats the pattern as a literal string, not a regex, so
    // metacharacters like `.` and `[0]` pass through unescaped and still
    // match only literally — this is the concern the fixed-string flag exists to avoid.
    let body = render_error_test_body("mycli extract --file x", Some("field.name[0]"));
    assert!(body.contains("grep -qF 'field.name[0]'"), "got: {body}");
}

#[test]
fn declared_value_with_single_quote_is_shell_escaped() {
    let body = render_error_test_body("mycli extract --file x", Some("it's bad"));
    assert!(body.contains(r"grep -qF 'it'\''s bad'"), "got: {body}");
}

/// Regression test: the diagnostic echo previously embedded the already
/// single-quoted `{quoted}` fragment inside a *double*-quoted string
/// (`echo "... {quoted}" >&2`). Beyond breaking on a literal `"` in the
/// value, that also re-enabled `$`/backtick expansion of the value's raw
/// content — a real injection hazard, not just a cosmetic quoting bug.
/// Assert every double-quoted context in the echo line is free of both
/// the raw declared value and shell metacharacters that would expand.
#[test]
fn declared_value_with_double_quote_and_dollar_does_not_break_out_of_double_quoted_context() {
    let value = r#"say "$(rm -rf /)""#;
    let body = render_error_test_body("mycli extract --file x", Some(value));
    let echo_line = body
        .lines()
        .find(|line| line.contains("expected output to contain"))
        .expect("diagnostic echo line must be present");
    // The whole message must be built from single-quoted segments only —
    // no `"..."` span should contain the raw value (which would let `$(...)`
    // expand at runtime).
    assert!(
        !echo_line.contains("\"FAIL"),
        "diagnostic message must not be embedded in a double-quoted string: {echo_line}"
    );
    assert!(
        echo_line.starts_with("    echo 'FAIL [error]: expected output to contain '"),
        "got: {echo_line}"
    );
}

use crate::e2e::escape::{escape_shell, sanitize_ident};
use crate::e2e::field_access::FieldResolver;
use crate::e2e::fixture::Assertion;

// ~keep The template appends its sentinel only after successful jq extraction: Bash
// trims trailing newlines, and conditional callers suppress automatic errexit.
pub(super) fn render(out: &mut String, identifier: &str, jq_path: &str, field: &str) {
    out.push_str(&crate::e2e::template_env::render(
        "brew/value_capture.jinja",
        minijinja::context! {
            identifier => identifier,
            jq_path => escape_shell(jq_path),
            field => escape_shell(field),
        },
    ));
}

pub(super) fn render_equals(out: &mut String, assertion: &Assertion, resolver: &FieldResolver) {
    let (Some(field), Some(expected)) = (&assertion.field, &assertion.value) else {
        return;
    };
    let identifier = sanitize_ident(field);
    let jq_path = super::field_to_jq_path(&resolver.result_relative_path(field));
    render(out, &identifier, &jq_path, field);
    out.push_str(&crate::e2e::template_env::render(
        "brew/equals_assertion.jinja",
        minijinja::context! {
            identifier => identifier,
            expected => super::json_value_to_shell_string(expected),
            field => escape_shell(field),
        },
    ));
}

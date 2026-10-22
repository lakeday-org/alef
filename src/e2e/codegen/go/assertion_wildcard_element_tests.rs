//! Regression coverage for the element accessor a wildcard (`container[].field`) fixture path
//! expands to in the Go e2e generator.
//!
//! Split into its own file rather than added to `go/assertions.rs`: that file sits at its recorded
//! ceiling in `tests/file_size_baseline.txt`, so new coverage goes into a fresh module instead of
//! growing it (see `file-modularization` in CLAUDE.md). ~keep
//!
//! The defect: `render_wildcard_assertion` splits `records[].kind` into the container `records`
//! and the element half `kind`, then built the loop body with `FieldResolver::accessor`, which
//! anchors a path against the call's RESULT type. `kind` is not declared on the root, so the
//! envelope rescue prefixed it through the `result_fields` entry that does reach it and handed
//! back `records[0].kind`. Rendered against `e`, which is ALREADY an element, that is
//! `e.Records[0].Kind` — the container path applied a second time, addressing a field the Go
//! element struct does not have. `element_accessor` anchors at the element instead.

use std::collections::{HashMap, HashSet};

use super::assertions::{AssertionRenderContext, render_assertion};
use crate::e2e::codegen::wildcard_element_fixture::{
    WILDCARD_FIELD, assert_container_accessor_appears_once, assert_element_relative, contains_assertion,
    envelope_resolver, report_resolver,
};
use crate::e2e::field_access::FieldResolver;

const CONTAINER_ACCESSOR: &str = ".Records";

fn render_assertion_with(resolver: &FieldResolver, assertion: &crate::e2e::fixture::Assertion) -> String {
    let mut out = String::new();
    let context = AssertionRenderContext {
        effective_result_var: "result",
        import_alias: "sample",
        field_resolver: resolver,
        optional_locals: &HashMap::new(),
        numeric_scalar_fields: &HashSet::new(),
        presence_checked_fields: &HashSet::new(),
        result_is_simple: false,
        result_is_array: false,
        is_streaming: false,
        streaming_item_type: None,
    };
    render_assertion(&mut out, &context, assertion);
    out
}

#[test]
fn wildcard_loop_body_is_relative_to_the_element_binding() {
    let rendered = render(&report_resolver("go"));

    assert_element_relative(&rendered, "e.Kind", "e.Records[0].Kind");
    assert_container_accessor_appears_once(&rendered, CONTAINER_ACCESSOR);
}

/// The container half must keep resolving against the result variable — the fix must not turn
/// into "never anchor", which would still satisfy the assertion above.
#[test]
fn wildcard_container_stays_anchored_to_the_result_variable() {
    let rendered = render(&report_resolver("go"));

    assert!(
        rendered.contains("for _, e := range result.Records {"),
        "container half must iterate the result variable's own field, got: {rendered}"
    );
}

/// The stronger container control: on an envelope root the container is reachable only THROUGH
/// the `result_fields` projection, so dropping the anchoring from `accessor` — rather than only
/// from the element half — renders `result.Records`, a member the envelope does not declare.
#[test]
fn wildcard_container_keeps_its_envelope_projection() {
    let rendered = render(&envelope_resolver("go"));

    assert!(
        rendered.contains("for _, e := range result.Results[0].Records {"),
        "container half must keep the result-anchored envelope projection, got: {rendered}"
    );
    assert_container_accessor_appears_once(&rendered, CONTAINER_ACCESSOR);
}

fn render(resolver: &FieldResolver) -> String {
    render_assertion_with(resolver, &contains_assertion(WILDCARD_FIELD))
}

#[test]
fn optional_wildcard_strings_dereference_only_present_leaves() {
    use crate::e2e::codegen::wildcard_element_fixture::{WILDCARD_NAME_FIELD, optional_name_resolver};
    let mut failures = Vec::new();
    let mut checked = 0;
    for kind in ["contains", "not_contains", "contains_all", "not_empty"] {
        for multiple in [false, true] {
            if kind == "contains_all" && !multiple {
                continue;
            }
            let assertion = crate::e2e::fixture::Assertion {
                assertion_type: kind.into(),
                field: Some(WILDCARD_NAME_FIELD.into()),
                value: (!multiple).then(|| serde_json::json!("")),
                values: multiple.then(|| vec![serde_json::json!(""), serde_json::json!("t")]),
                ..Default::default()
            };
            let rendered = render_assertion_with(&optional_name_resolver("go"), &assertion);
            checked += 1;
            if !rendered.contains("e.Name != nil &&") || !rendered.contains("*e.Name") {
                failures.push(format!("{kind}/{multiple}: {rendered}"));
            }
        }
    }
    assert_eq!(checked, 7);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[ignore = "requires the Go toolchain"]
fn optional_wildcard_generated_go_compiles_and_runs() {
    use crate::e2e::codegen::wildcard_element_fixture::{
        compile_and_run_wildcard_program, optional_name_resolver, wildcard_runtime_cases,
    };
    let cases: Vec<_> = wildcard_runtime_cases("go")
        .iter()
        .enumerate()
        .map(|(index, (assertion, name, passes))| {
            crate::alef_context! {
                index => index,
                name_literal => name.map(|name| format!("{name:?}")),
                passes => passes,
                assertion => render_assertion_with(&optional_name_resolver("go"), assertion),
            }
        })
        .collect();
    assert_eq!(cases.len(), 60);
    let source = crate::e2e::template_env::render(
        "go/optional_wildcard_runtime.go.jinja",
        crate::alef_context! { cases => cases },
    );
    assert_eq!(source.matches("func case").count(), 60);
    assert_eq!(source.matches("panic(\"case ").count(), 60);
    compile_and_run_wildcard_program("go", &source);
}

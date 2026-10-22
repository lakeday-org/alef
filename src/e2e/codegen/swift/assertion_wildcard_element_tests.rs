//! Regression coverage for the element accessor a wildcard (`container[].field`) fixture path
//! expands to in the Swift e2e generator.
//!
//! ~keep Swift was reported CLEAN by a survey of one consumer corpus's generated output. It is not
//! structurally clean: BOTH of its wildcard element sites — `accessors.rs`'s
//! `swift_traversal_contains_assert` and the `not_empty` arm in `assertions.rs` — routed the
//! element half through the same result-anchored `FieldResolver::accessor` the six
//! confirmed-defective backends used, against a resolver its `test_method.rs` anchors at the
//! call's declared result type exactly as they do. That corpus simply produced no `result_fields`
//! envelope rescue for this backend. The fixture below supplies one, and the assertions fail
//! against the pre-fix generator — an accident, not an immunity.
//!
//! Pre-fix the closure body was `$0.records()[0].kind()`: the container path applied a second
//! time against a binding that is already an element.

use std::collections::HashMap;

use super::assertions::render_assertion;
use crate::e2e::codegen::wildcard_element_fixture::{
    WILDCARD_FIELD, WILDCARD_NAME_FIELD, assert_container_accessor_appears_once, assert_element_relative,
    contains_assertion, envelope_resolver, not_empty_assertion, report_resolver,
};
use crate::e2e::field_access::FieldResolver;
use crate::e2e::fixture::Assertion;

const CONTAINER_ACCESSOR: &str = ".records()";

fn render(resolver: &FieldResolver, assertion: &Assertion) -> String {
    let mut out = String::new();
    render_assertion(
        &mut out,
        assertion,
        "result",
        resolver,
        false,
        false,
        false,
        false,
        &HashMap::new(),
        false,
        false,
        "Sample",
    );
    out
}

#[test]
fn wildcard_contains_closure_body_is_relative_to_the_element_binding() {
    let rendered = render(&report_resolver("swift"), &contains_assertion(WILDCARD_FIELD));

    assert_element_relative(&rendered, "$0.kind().toString()", "$0.records()[0].kind()");
    assert_container_accessor_appears_once(&rendered, CONTAINER_ACCESSOR);
}

/// The `not_empty` arm builds its element accessor at its own call site in `assertions.rs`
/// rather than through `swift_traversal_contains_assert`, so a fix applied to one site only
/// would leave this red. ~keep
#[test]
fn wildcard_not_empty_closure_body_is_also_element_relative() {
    let rendered = render(&report_resolver("swift"), &not_empty_assertion(WILDCARD_NAME_FIELD));

    assert_element_relative(&rendered, "$0.name().toString()", "$0.records()[0].name()");
    assert_container_accessor_appears_once(&rendered, CONTAINER_ACCESSOR);
}

/// The container half must keep resolving against the result variable — the fix must not turn
/// into "never anchor", which would still satisfy the assertions above.
#[test]
fn wildcard_container_stays_anchored_to_the_result_variable() {
    let rendered = render(&report_resolver("swift"), &contains_assertion(WILDCARD_FIELD));

    assert!(
        rendered.contains("result.records().contains(where: {"),
        "container half must quantify over the result variable's own field, got: {rendered}"
    );
}

/// The stronger container control: on an envelope root the container is reachable only THROUGH
/// the `result_fields` projection, so dropping the anchoring from `accessor` — rather than only
/// from the element half — renders `result.records()`, a member the envelope does not declare.
#[test]
fn wildcard_container_keeps_its_envelope_projection() {
    let rendered = render(&envelope_resolver("swift"), &contains_assertion(WILDCARD_FIELD));

    assert!(
        rendered.contains("result.results()[0].records().contains(where: {"),
        "container half must keep the result-anchored envelope projection, got: {rendered}"
    );
    assert_container_accessor_appears_once(&rendered, CONTAINER_ACCESSOR);
}

#[test]
fn optional_wildcard_strings_never_match_absent_leaves() {
    use crate::e2e::codegen::wildcard_element_fixture::optional_name_resolver;
    let mut failures = Vec::new();
    let mut checked = 0;
    for kind in ["contains", "not_contains", "contains_all", "not_empty"] {
        let assertion = Assertion {
            assertion_type: kind.into(),
            field: Some(WILDCARD_NAME_FIELD.into()),
            value: Some(serde_json::json!("")),
            values: Some(vec![serde_json::json!(""), serde_json::json!("t")]),
            ..Default::default()
        };
        let rendered = render(&optional_name_resolver("swift"), &assertion);
        checked += 1;
        if !rendered.contains("$0.name?") || !(rendered.contains("== true") || rendered.contains("== false")) {
            failures.push(format!("{kind}: {rendered}"));
        }
    }
    assert_eq!(checked, 4);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[ignore = "requires the Swift toolchain"]
fn optional_wildcard_generated_swift_compiles_and_runs() {
    use crate::e2e::codegen::wildcard_element_fixture::{
        compile_and_run_wildcard_program, optional_name_resolver, wildcard_runtime_cases,
    };
    let cases: Vec<_> = wildcard_runtime_cases("swift")
        .iter()
        .enumerate()
        .map(|(index, (assertion, name, passes))| {
            crate::alef_context! {
                index => index,
                name_literal => name.map(|name| format!("{name:?}")),
                passes => passes,
                assertion => render(&optional_name_resolver("swift"), assertion),
            }
        })
        .collect();
    assert_eq!(cases.len(), 60);
    let source = crate::e2e::template_env::render(
        "swift/optional_wildcard_runtime.swift.jinja",
        crate::alef_context! { cases => cases },
    );
    assert_eq!(source.matches("func case").count(), 60);
    assert_eq!(source.matches("precondition(case").count(), 60);
    compile_and_run_wildcard_program("swift", &source);
}

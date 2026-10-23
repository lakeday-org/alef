use std::collections::{BTreeMap, HashMap, HashSet};

use super::*;
use crate::e2e::field_access::PhpGetterMap;

fn empty_resolver() -> FieldResolver {
    FieldResolver::new(
        &HashMap::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
    )
}

/// `data`'s Rust type is `Option<DataNode>` -- a non-scalar struct, which ext-php-rs
/// exposes only through a `#[php(getter)]` method, not a plain property. Build the same
/// shape of `PhpGetterMap` alef's own codegen would derive from the IR, using the
/// "owner type unknown" bare-name-union fallback (`needs_getter`'s legacy path) so the
/// test doesn't have to also wire a full type graph.
fn optional_getter_resolver(field: &str) -> FieldResolver {
    let optional: HashSet<String> = [field.to_string()].into_iter().collect();
    let getter_map = PhpGetterMap {
        getters: HashMap::from([("ProcessResult".to_string(), HashSet::from([field.to_string()]))]),
        ..PhpGetterMap::default()
    };
    FieldResolver::new_with_error_aliases(
        &HashMap::new(),
        &optional,
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashMap::new(),
    )
    .with_php_getter_map(getter_map)
}

fn is_true_assertion(field: &str) -> Assertion {
    Assertion {
        assertion_type: "is_true".to_string(),
        field: Some(field.to_string()),
        ..Assertion::default()
    }
}

fn render(resolver: &FieldResolver, assertion: &Assertion) -> String {
    let mut out = String::new();
    let getter_map = PhpGetterMap::default();
    let lowering = super::super::enum_variant_access::PhpEnumLowering::from_enums(&[]);
    render_assertion(
        &mut out,
        assertion,
        "result",
        resolver,
        false,
        false,
        &BTreeMap::new(),
        false,
        &super::super::enum_variant_access::PhpVariantAccess::new(&getter_map, &lowering),
    );
    out
}

/// `Option<DataNode>` presence: before the fix `field_expr` was unconditionally wrapped
/// `($result->getData() ?? null)` for every assertion type on an optional field, and
/// `is_true` passed that straight to `assertTrue`, which PHPUnit's declared `bool`
/// parameter type rejects at runtime for a present non-bool value.
#[test]
fn is_true_on_optional_struct_field_checks_presence() {
    let out = render(&optional_getter_resolver("data"), &is_true_assertion("data"));
    assert_eq!(
        out,
        "            $this->assertTrue(($result->getData() ?? null) !== null);\n"
    );
}

#[test]
fn is_false_on_optional_struct_field_checks_absence() {
    let out = render(
        &optional_getter_resolver("data"),
        &Assertion {
            assertion_type: "is_false".to_string(),
            field: Some("data".to_string()),
            ..Assertion::default()
        },
    );
    assert_eq!(
        out,
        "            $this->assertTrue(($result->getData() ?? null) === null);\n"
    );
}

/// A follow-on member access through the same optional field must null-safe navigate:
/// before the fix `render_php_with_getters` never consulted `optional_fields`, so a
/// nested path emitted `$result->getData()->kind` with no `?->`.
#[test]
fn equals_on_nested_field_through_optional_parent_null_safe_navigates() {
    let out = render(
        &optional_getter_resolver("data"),
        &Assertion {
            assertion_type: "equals".to_string(),
            field: Some("data.kind".to_string()),
            value: Some(serde_json::json!("KeyValue")),
            ..Assertion::default()
        },
    );
    assert!(out.contains("$result->getData()?->kind"), "got: {out}");
}

#[test]
fn is_true_on_non_optional_field_is_unchanged() {
    let out = render(&empty_resolver(), &is_true_assertion("active"));
    assert_eq!(out, "            $this->assertTrue($result->active);\n");
}

/// IR-oracle wiring regression (alef task #64): a field that is IR-reachable
/// (present, non-`binding_excluded`, on some IR type) but missing from the
/// hand-maintained `result_fields` config must still render a real assertion,
/// not a "skipped: field not available" comment — `php/test_method.rs` (shared
/// by `php` and `php_ext`) now threads `FieldResolver::ir_field_sets(type_defs)`
/// into `with_ir_fields`. ~keep
#[test]
fn php_ir_reachable_field_absent_from_result_fields_is_not_skipped() {
    let reachable: HashSet<String> = ["data".to_string()].into_iter().collect();
    let resolver = FieldResolver::new(
        &HashMap::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
    )
    .with_ir_fields(reachable, HashSet::new(), HashSet::new());
    let assertion = Assertion {
        assertion_type: "equals".to_string(),
        field: Some("data".to_string()),
        value: Some(serde_json::Value::String("hello".to_string())),
        ..Default::default()
    };
    let mut out = String::new();
    render_assertion(
        &mut out,
        &assertion,
        "result",
        &resolver,
        false,
        false,
        &BTreeMap::new(),
        false,
        &super::super::enum_variant_access::PhpVariantAccess::none(),
    );
    assert!(!out.contains("skipped"), "got: {out}");
}

/// The negative-control half of the same regression: `internal_diagnostics`
/// represents a field carrying `#[doc(hidden)]` or `#[cfg_attr(alef,
/// alef(skip))]` in the real struct (a genuine `binding_excluded` field) —
/// NOT `#[serde(skip)]`, which alone does not exclude a field from the
/// binding surface. Even though it is listed in `result_fields` (a stale/
/// wrong config entry), the IR must still win and reject it. ~keep
#[test]
fn php_ir_excluded_field_present_in_result_fields_is_still_skipped() {
    let result_fields: HashSet<String> = ["internal_diagnostics".to_string()].into_iter().collect();
    let excluded: HashSet<String> = ["internal_diagnostics".to_string()].into_iter().collect();
    let resolver = FieldResolver::new(
        &HashMap::new(),
        &HashSet::new(),
        &result_fields,
        &HashSet::new(),
        &HashSet::new(),
    )
    .with_ir_fields(HashSet::new(), excluded, HashSet::new());
    let assertion = Assertion {
        assertion_type: "equals".to_string(),
        field: Some("internal_diagnostics".to_string()),
        value: Some(serde_json::Value::String("hello".to_string())),
        ..Default::default()
    };
    let mut out = String::new();
    render_assertion(
        &mut out,
        &assertion,
        "result",
        &resolver,
        false,
        false,
        &BTreeMap::new(),
        false,
        &super::super::enum_variant_access::PhpVariantAccess::none(),
    );
    assert!(out.contains("skipped"), "got: {out}");
}

/// Regression test for a one-sided-trim bug: `trim()` wrapped the actual value while the
/// fixture `expected` literal was emitted verbatim. Fixture expectations may legitimately
/// end in `\n`, so trimming only one side made those assertions impossible to satisfy —
/// and trimming both would silently mask real trailing-whitespace regressions. Equals is
/// exact: neither side is normalized.
/// Control for the trim fix: the tightened contract must still DISCRIMINATE values that
/// differ only in trailing whitespace. If either side were normalized, the emitted
/// assertion for "hello\n" and for "hello" would be identical and a real trailing-newline
/// regression would pass unnoticed.
#[test]
fn render_assertion_equals_still_discriminates_trailing_whitespace() {
    let render_for = |value: &str| {
        let resolver = empty_resolver();
        let assertion = Assertion {
            assertion_type: "equals".to_string(),
            field: None,
            value: Some(serde_json::Value::String(value.into())),
            ..Default::default()
        };
        let mut out = String::new();
        render_assertion(
            &mut out,
            &assertion,
            "result",
            &resolver,
            true,
            false,
            &BTreeMap::new(),
            false,
            &super::super::enum_variant_access::PhpVariantAccess::none(),
        );
        out
    };
    let emitted = render_for("hello\n");
    // The actual side must be the bare expression: any normalizing call (trim/strip/
    // case-folding) wrapped around it would silently accept a mismatched value.
    assert_eq!(
        emitted, "            $this->assertEquals(\"hello\\n\", $result);\n",
        "emitted assertion drifted: {emitted}"
    );
    // And a value differing only by the trailing newline must still produce a
    // different expectation, proving trailing whitespace is discriminated.
    assert_ne!(
        emitted,
        render_for("hello"),
        "trailing newline must still change the emitted assertion"
    );
}

#[test]
fn render_assertion_equals_string_compares_exactly_without_trim() {
    let resolver = empty_resolver();
    let assertion = Assertion {
        assertion_type: "equals".to_string(),
        field: None,
        value: Some(serde_json::Value::String("hello\n".into())),
        ..Default::default()
    };
    let mut out = String::new();
    render_assertion(
        &mut out,
        &assertion,
        "result",
        &resolver,
        true,
        false,
        &BTreeMap::new(),
        false,
        &super::super::enum_variant_access::PhpVariantAccess::none(),
    );
    assert!(!out.contains("trim("), "equals must not trim either side; got: {out}");
    assert!(out.contains("assertEquals("), "got: {out}");
}

fn wildcard_resolver() -> FieldResolver {
    FieldResolver::new(
        &HashMap::new(),
        &HashSet::new(),
        &HashSet::from(["links".to_string(), "pages".to_string()]),
        &HashSet::from(["links".to_string(), "pages".to_string()]),
        &HashSet::new(),
    )
}

fn render_wildcard(assertion_type: &str, field: &str) -> String {
    let assertion = Assertion {
        assertion_type: assertion_type.to_string(),
        field: Some(field.to_string()),
        value: if assertion_type == "not_empty" {
            None
        } else {
            Some(serde_json::Value::String("internal".to_string()))
        },
        ..Default::default()
    };
    let mut out = String::new();
    render_assertion(
        &mut out,
        &assertion,
        "result",
        &wildcard_resolver(),
        false,
        false,
        &BTreeMap::new(),
        false,
        &super::super::enum_variant_access::PhpVariantAccess::none(),
    );
    out
}

/// A bracket-wildcard fixture path means "every element", so the emitted PHP
/// must quantify with array_filter over the whole array.
#[test]
fn php_wildcard_contains_emits_quantifier_over_all_elements() {
    let out = render_wildcard("contains", "links[].link_type");
    assert!(
        out.contains(
            "$this->assertTrue((bool)array_filter($result->links, fn($e) => str_contains((string)$e->linkType, \"internal\")));"
        ),
        "expected an any-element quantifier, got:\n{out}"
    );
}

/// THE CANARY. A fixture whose match lives only in element 1 is satisfied by an
/// any-element quantifier and missed by an index-0 lookup. This unit test observes
/// the emitted source rather than executing it, so it pins the property that makes
/// the runtime difference: the wildcard must NOT lower to a single-element access.
/// Pre-fix the wildcard rendered `$result->links[0]->linkType`, which reads element 0
/// only and reports a false green; this assertion is red then.
#[test]
fn php_wildcard_does_not_collapse_to_element_zero() {
    let out = render_wildcard("contains", "links[].link_type");
    assert!(
        !out.contains("[0]"),
        "wildcard must not lower to a single-element access, got:\n{out}"
    );
}

/// Regression lock: an explicit numeric index is not a wildcard and must keep
/// resolving to that exact element.
#[test]
fn php_explicit_index_still_resolves_to_element_zero() {
    let out = render_wildcard("contains", "links[0].link_type");
    assert!(
        out.contains("$result->links[0]->linkType"),
        "explicit index 0 must keep its index-preserving accessor, got:\n{out}"
    );
    assert!(
        !out.contains("array_filter"),
        "explicit index must not become a quantifier, got:\n{out}"
    );
}

/// `wildcard_split` consumes the first `[].` only, so before the guard the `array_filter`
/// ranged over `pages` while its closure read `$e->links[0]->url` — a whole-array claim
/// that only ever inspected element zero of the inner array. Pre-guard this test fails:
/// the skip line is absent and a quantifier over `[0]` is present. ~keep
#[test]
fn nested_wildcard_should_emit_a_visible_skip_rather_than_an_index_zero_check() {
    let out = render_wildcard("contains", "pages[].links[].url");
    assert_eq!(
        out, "        // skipped: nested array-wildcard field 'pages[].links[].url' not supported\n",
        "got:\n{out}"
    );
}

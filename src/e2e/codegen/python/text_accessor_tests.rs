//! A Python text assertion on a `fields_display_as_text` field reads the value through the
//! binding's `.text` accessor instead of `str()`.
//!
//! The generated helper `_alef_e2e_text` turns any object into text through `str()`. For an
//! untagged enum such as a message `content` (a text variant and a parts variant) the object's
//! string form is correct even when the `.text` getter returns `None`, so a broken getter
//! passed. These tests drive `render_test_function` with the `fields_display_as_text` config,
//! the same key the Go, Java, C# and PHP generators read. A `None` accessor fails the assertion,
//! and a field that is not an enum field keeps the exact comparison the standard assertions use.

use std::collections::{HashMap, HashSet};

use crate::core::config::ResolvedCrateConfig;
use crate::core::ir::{EnumDef, EnumVariant, FieldDef, FunctionDef, TypeDef, TypeRef};
use crate::e2e::config::{CallConfig, E2eConfig};
use crate::e2e::fixture::{Assertion, Fixture};

use super::test_function::{RenderTestFunctionContext, render_test_function};

fn content_enum() -> EnumDef {
    EnumDef {
        name: "MessageContent".to_string(),
        serde_untagged: true,
        variants: vec![
            EnumVariant {
                name: "Text".to_string(),
                fields: vec![FieldDef {
                    name: "_0".to_string(),
                    ty: TypeRef::String,
                    ..FieldDef::default()
                }],
                ..EnumVariant::default()
            },
            EnumVariant {
                name: "Parts".to_string(),
                fields: vec![FieldDef {
                    name: "_0".to_string(),
                    ty: TypeRef::Vec(Box::new(TypeRef::String)),
                    ..FieldDef::default()
                }],
                ..EnumVariant::default()
            },
        ],
        ..EnumDef::default()
    }
}

/// `reply` returns `Reply { content: Option<MessageContent> }`.
fn render_assertion_for(assertion: Assertion, display_as_text: bool) -> String {
    let type_defs = vec![TypeDef {
        name: "Reply".to_string(),
        fields: vec![
            FieldDef {
                name: "content".to_string(),
                ty: TypeRef::Optional(Box::new(TypeRef::Named("MessageContent".to_string()))),
                optional: true,
                ..FieldDef::default()
            },
            // Listed as display-as-text but not an enum field: the standard assertions compare
            // such a field exactly.
            FieldDef {
                name: "label".to_string(),
                ty: TypeRef::Optional(Box::new(TypeRef::String)),
                optional: true,
                ..FieldDef::default()
            },
        ],
        ..TypeDef::default()
    }];
    let enums = vec![content_enum()];
    let functions = vec![FunctionDef {
        name: "reply".to_string(),
        return_type: TypeRef::Named("Reply".to_string()),
        ..FunctionDef::default()
    }];
    let mut e2e_config = E2eConfig::default();
    e2e_config.calls.insert(
        "reply".to_string(),
        CallConfig {
            function: "reply".to_string(),
            result_var: "result".to_string(),
            ..CallConfig::default()
        },
    );
    e2e_config.fields_optional.insert("content".to_string());
    e2e_config.fields_optional.insert("label".to_string());
    if display_as_text {
        e2e_config.fields_display_as_text.insert("content".to_string());
        e2e_config.fields_display_as_text.insert("label".to_string());
        e2e_config.fields_display_as_text.insert("items[].content".to_string());
    }
    let fixture = Fixture {
        id: "reply_text".to_string(),
        description: "Reply text".to_string(),
        call: Some("reply".to_string()),
        assertions: vec![assertion],
        ..Fixture::default()
    };
    let config = ResolvedCrateConfig::default();
    let enum_fields = HashMap::new();
    let handle_nested_types = HashMap::new();
    let handle_dict_types = HashSet::new();
    let convertible_types = ahash::AHashSet::new();
    let options_wrapped_types = HashSet::new();
    let mut out = String::new();
    render_test_function(
        &mut out,
        &fixture,
        RenderTestFunctionContext {
            e2e_config: &e2e_config,
            config: &config,
            type_defs: &type_defs,
            enums: &enums,
            functions: &functions,
            errors: &[],
            options_type: None,
            options_via: "kwargs",
            enum_fields: &enum_fields,
            handle_nested_types: &handle_nested_types,
            handle_dict_types: &handle_dict_types,
            force_bind_result: false,
            convertible_types: &convertible_types,
            crate_has_serde: false,
            options_wrapped_types: &options_wrapped_types,
        },
    );
    out
}

fn text_assertion_on(field: &str, kind: &str, value: serde_json::Value) -> Assertion {
    let mut assertion = Assertion {
        assertion_type: kind.to_string(),
        field: Some(field.to_string()),
        ..Assertion::default()
    };
    if kind == "contains_all" || kind == "contains_any" {
        assertion.values = Some(vec![value]);
    } else {
        assertion.value = Some(value);
    }
    assertion
}

fn text_assertion(kind: &str, value: serde_json::Value) -> Assertion {
    text_assertion_on("content", kind, value)
}

fn hello() -> serde_json::Value {
    serde_json::json!("hello")
}

#[test]
fn equals_reads_the_text_accessor_and_requires_the_value_to_be_present() {
    let out = render_assertion_for(text_assertion("equals", hello()), true);
    assert!(out.contains("    assert result.content is not None\n"), "{out}");
    assert!(out.contains("    assert result.content.text is not None\n"), "{out}");
    assert!(
        out.contains("    assert result.content.text.lower() == \"hello\".lower()\n"),
        "{out}"
    );
    assert!(!out.contains("_alef_e2e_text"), "{out}");
}

#[test]
fn a_field_outside_the_config_keeps_the_string_form() {
    let out = render_assertion_for(text_assertion("equals", hello()), false);
    assert!(
        out.contains("    assert _alef_e2e_text(result.content).lower() == \"hello\".lower()\n"),
        "{out}"
    );
    assert!(!out.contains(".text"), "{out}");
}

#[test]
fn every_text_comparison_reads_the_text_accessor() {
    let cases = [
        (
            "contains",
            "    assert \"hello\".lower() in result.content.text.lower()\n",
        ),
        (
            "contains_all",
            "    assert \"hello\".lower() in result.content.text.lower()\n",
        ),
        (
            "contains_any",
            "    assert any(v.lower() in result.content.text.lower() for v in [\"hello\"])\n",
        ),
        (
            "not_contains",
            "    assert result.content is None or \"hello\".lower() not in result.content.text.lower()\n",
        ),
        ("starts_with", "    assert result.content.text.startswith(\"hello\")\n"),
        ("ends_with", "    assert result.content.text.endswith(\"hello\")\n"),
    ];
    for (kind, expected) in cases {
        let out = render_assertion_for(text_assertion(kind, hello()), true);
        assert!(out.contains(expected), "{kind}: {out}");
        assert!(!out.contains("str(result.content)"), "{kind}: {out}");
    }
}

#[test]
fn a_field_that_is_not_an_enum_field_keeps_the_exact_comparison() {
    // The standard assertions compare an enum field case-insensitively and any other field
    // exactly, so a reply in the wrong case must still fail on `label`.
    let cases = [
        ("equals", "    assert result.label.text == \"hello\"\n"),
        ("contains", "    assert \"hello\" in result.label.text\n"),
        ("contains_all", "    assert \"hello\" in result.label.text\n"),
        (
            "contains_any",
            "    assert any(v in result.label.text for v in [\"hello\"])\n",
        ),
        (
            "not_contains",
            "    assert result.label is None or \"hello\" not in result.label.text\n",
        ),
        ("starts_with", "    assert result.label.text.startswith(\"hello\")\n"),
        ("ends_with", "    assert result.label.text.endswith(\"hello\")\n"),
    ];
    for (kind, expected) in cases {
        let out = render_assertion_for(text_assertion_on("label", kind, hello()), true);
        assert!(out.contains(expected), "{kind}: {out}");
        assert!(!out.contains(".lower()"), "{kind}: {out}");
    }
}

#[test]
fn an_accessor_that_returns_none_fails_every_text_comparison() {
    // `None` must stay distinct from the empty string: an empty expected value would otherwise
    // match a getter that returns nothing.
    for kind in ["equals", "contains", "not_contains", "starts_with", "ends_with"] {
        for value in [hello(), serde_json::json!("")] {
            let out = render_assertion_for(text_assertion(kind, value), true);
            let none_check = out
                .find("result.content.text is not None\n")
                .unwrap_or_else(|| panic!("{kind}: no check that the accessor is not None: {out}"));
            let comparison = out
                .rfind("result.content.text")
                .unwrap_or_else(|| panic!("{kind}: no comparison: {out}"));
            assert!(none_check < comparison, "{kind}: {out}");
            assert!(!out.contains("_alef_e2e_text"), "{kind}: {out}");
        }
    }
}

#[test]
fn contains_all_checks_every_literal() {
    let mut assertion = text_assertion("contains_all", hello());
    assertion.values = Some(vec![serde_json::json!("alpha"), serde_json::json!("beta")]);
    let out = render_assertion_for(assertion, true);
    assert!(
        out.contains("    assert \"alpha\".lower() in result.content.text.lower()\n"),
        "{out}"
    );
    assert!(
        out.contains("    assert \"beta\".lower() in result.content.text.lower()\n"),
        "{out}"
    );
}

#[test]
fn an_empty_expected_list_keeps_the_standard_rendering() {
    let mut contains_all = text_assertion("contains_all", hello());
    contains_all.values = Some(vec![]);
    let mut contains_any = text_assertion("contains_any", hello());
    contains_any.values = Some(vec![]);
    let mut equals = text_assertion("equals", hello());
    equals.value = None;
    for assertion in [contains_all, contains_any, equals] {
        let kind = assertion.assertion_type.clone();
        let out = render_assertion_for(assertion, true);
        assert!(!out.contains(".text"), "{kind}: {out}");
    }
}

#[test]
fn a_wildcard_path_keeps_the_string_form() {
    let out = render_assertion_for(text_assertion_on("items[].content", "contains", hello()), true);
    assert!(out.contains("any(\"hello\" in str("), "{out}");
    assert!(!out.contains(".text"), "{out}");
}

#[test]
fn presence_and_size_assertions_keep_the_whole_value() {
    // A parts value has no text, but it is still present: `not_empty` must keep passing.
    let out = render_assertion_for(
        Assertion {
            assertion_type: "not_empty".to_string(),
            field: Some("content".to_string()),
            ..Assertion::default()
        },
        true,
    );
    assert!(out.contains("assert result.content is not None and "), "{out}");
    assert!(!out.contains(".text"), "{out}");
}

#[test]
fn a_non_text_expected_value_keeps_the_whole_value() {
    let out = render_assertion_for(text_assertion("equals", serde_json::json!(7)), true);
    assert!(out.contains("    assert result.content == 7\n"), "{out}");
    assert!(!out.contains(".text"), "{out}");
}

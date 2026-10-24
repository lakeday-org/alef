//! A path that steps through an untagged union into its list payload reads the wrapper's JSON
//! tree. The wrapper class has no accessor for a variant.
//!
//! Types are neutral: `Reply { choices: Vec<Choice> }`, `Choice { message: Message }`,
//! `Message { content: Option<Body> }`, `Body::{Text(String), Parts(Vec<Piece>)}` (untagged),
//! `Piece::{Text { text }}` (internally tagged).

use super::test_method::render_test_method;
use crate::core::config::ResolvedCrateConfig;
use crate::core::ir::{EnumDef, EnumVariant, FieldDef, FunctionDef, TypeDef, TypeRef};
use crate::e2e::config::{CallConfig, E2eConfig};
use crate::e2e::fixture::{Assertion, Fixture};

fn field(name: &str, ty: TypeRef, optional: bool) -> FieldDef {
    FieldDef {
        name: name.to_string(),
        ty,
        optional,
        ..FieldDef::default()
    }
}

fn named(name: &str) -> TypeRef {
    TypeRef::Named(name.to_string())
}

fn ir() -> (Vec<TypeDef>, Vec<EnumDef>, Vec<FunctionDef>) {
    let def = |name: &str, fields: Vec<FieldDef>| TypeDef {
        name: name.to_string(),
        fields,
        ..TypeDef::default()
    };
    let types = vec![
        def(
            "Reply",
            vec![field("choices", TypeRef::Vec(Box::new(named("Choice"))), false)],
        ),
        def("Choice", vec![field("message", named("Message"), false)]),
        def(
            "Message",
            vec![field("content", TypeRef::Optional(Box::new(named("Body"))), true)],
        ),
    ];
    let enums = vec![
        EnumDef {
            name: "Body".to_string(),
            serde_untagged: true,
            variants: vec![
                EnumVariant {
                    name: "Text".to_string(),
                    is_tuple: true,
                    fields: vec![field("_0", TypeRef::String, false)],
                    ..EnumVariant::default()
                },
                EnumVariant {
                    name: "Parts".to_string(),
                    is_tuple: true,
                    fields: vec![field("_0", TypeRef::Vec(Box::new(named("Piece"))), false)],
                    ..EnumVariant::default()
                },
            ],
            ..EnumDef::default()
        },
        EnumDef {
            name: "Piece".to_string(),
            serde_tag: Some("type".to_string()),
            serde_rename_all: Some("snake_case".to_string()),
            variants: vec![EnumVariant {
                name: "Text".to_string(),
                fields: vec![field("text", TypeRef::String, false)],
                ..EnumVariant::default()
            }],
            ..EnumDef::default()
        },
    ];
    let functions = vec![FunctionDef {
        name: "chat".to_string(),
        return_type: named("Reply"),
        ..FunctionDef::default()
    }];
    (types, enums, functions)
}

fn render(assertion_type: &str, path: &str, value: serde_json::Value) -> String {
    render_returning(assertion_type, path, value, "Reply")
}

fn render_returning(assertion_type: &str, path: &str, value: serde_json::Value, root: &str) -> String {
    let (types, enums, mut functions) = ir();
    functions[0].return_type = named(root);
    let fixture = Fixture {
        id: "parts".to_string(),
        description: "test".to_string(),
        assertions: vec![Assertion {
            assertion_type: assertion_type.to_string(),
            field: Some(path.to_string()),
            value: Some(value),
            ..Assertion::default()
        }],
        ..Fixture::default()
    };
    let config = E2eConfig {
        call: CallConfig {
            function: "chat".to_string(),
            result_var: "result".to_string(),
            ..CallConfig::default()
        },
        ..Default::default()
    };
    let mut out = String::new();
    render_test_method(
        &mut out,
        &fixture,
        "SampleClass",
        "",
        "",
        &[],
        None,
        false,
        &config,
        &std::collections::HashMap::new(),
        false,
        &[],
        &ResolvedCrateConfig::default(),
        &types,
        &enums,
        &functions,
        &[],
    );
    out
}

#[test]
fn the_count_of_the_parts_is_read_from_the_wrapper_json() {
    let rendered = render("count_equals", "choices[0].message.content.parts", serde_json::json!(2));
    assert!(
        rendered.contains("assertEquals(2, result.choices().get(0).message().content().value().size());"),
        "{rendered}"
    );
    assert!(!rendered.contains(".parts()"), "{rendered}");
}

#[test]
fn the_text_of_a_part_is_read_from_the_wrapper_json() {
    let rendered = render(
        "equals",
        "choices[0].message.content.parts[1].text",
        serde_json::json!(" world"),
    );
    assert!(
        rendered.contains("assertEquals(\" world\", result.choices().get(0).message().content().value().get(1).get(\"text\").asText());"),
        "{rendered}"
    );
}

#[test]
fn issue688_java_root_enum_collection_uses_json_wrapper() {
    for (kind, field, value, expected) in [
        ("count_equals", "parts", serde_json::json!(2), "result.value().size()"),
        (
            "equals",
            "parts[1].text",
            serde_json::json!(" world"),
            "result.value().get(1).get(\"text\").asText()",
        ),
    ] {
        let output = render_returning(kind, field, value, "Body");
        assert!(output.contains(expected), "{output}");
        assert!(output.contains("isArray()"), "{output}");
        assert!(!output.contains(".parts()"), "{output}");
    }
}

#[test]
fn issue688_java_standard_collection_checks_use_wrapper_json() {
    for (root, path) in [("Body", "parts"), ("Reply", "choices[0].message.content.parts")] {
        for (kind, value, fragment) in [
            ("count_min", serde_json::json!(2), ".size() >= 2"),
            ("is_empty", serde_json::Value::Null, ".isEmpty()"),
            ("not_empty", serde_json::Value::Null, ".isEmpty()"),
        ] {
            let output = render_returning(kind, path, value, root);
            assert!(!output.contains(".parts()"), "{output}");
            assert!(output.contains(".value().isArray()"), "{output}");
            assert!(output.contains(fragment), "{output}");
            assert!(!output.contains("Assumptions"), "{output}");
        }
    }
}

#[test]
fn issue688_java_unsupported_collection_checks_fail_explicitly() {
    for kind in ["contains", "future_kind"] {
        let output = render_returning(kind, "parts", serde_json::json!("needle"), "Body");
        assert!(!output.contains(".parts()"), "{output}");
        assert!(
            output.contains("fail(\"unsupported enum collection assertion"),
            "{output}"
        );
        assert!(!output.contains("Assumptions"), "{output}");
    }
}

#[test]
fn issue688_java_invalid_collection_counts_fail_without_narrowing() {
    for value in [
        serde_json::json!(u64::MAX),
        serde_json::json!(-1),
        serde_json::Value::Null,
    ] {
        let output = render_returning("count_min", "parts", value, "Body");
        assert!(
            output.contains("fail(\"invalid value for enum collection assertion"),
            "{output}"
        );
        assert!(!output.contains(".parts()"), "{output}");
        assert!(!output.contains("18446744073709551615"), "{output}");
    }
}

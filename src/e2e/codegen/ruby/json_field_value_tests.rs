use crate::backends::magnus::MagnusBackend;
use crate::core::backend::Backend;
use crate::core::config::ResolvedCrateConfig;
use crate::core::config::e2e::{ArgMapping, CallOverride};
use crate::core::ir::{ApiSurface, FieldDef, TypeDef, TypeRef};
use crate::e2e::config::E2eConfig;
use crate::e2e::fixture::Fixture;
use serde_json::{Value, json};

fn field(name: &str, ty: TypeRef) -> FieldDef {
    FieldDef {
        name: name.into(),
        ty,
        ..Default::default()
    }
}

fn cases() -> Vec<(&'static str, TypeRef, Value, &'static str)> {
    vec![
        ("object", TypeRef::Json, json!({"limit": 7}), r#"'{"limit":7}'"#),
        ("array", TypeRef::Json, json!([1, false]), "'[1,false]'"),
        ("string", TypeRef::Json, json!("hello"), r#"'"hello"'"#),
        ("number", TypeRef::Json, json!(7), "'7'"),
        ("boolean", TypeRef::Json, json!(false), "'false'"),
        ("required_null", TypeRef::Json, Value::Null, "'null'"),
        (
            "optional_null",
            TypeRef::Optional(Box::new(TypeRef::Json)),
            Value::Null,
            "nil",
        ),
        (
            "optional_object",
            TypeRef::Optional(Box::new(TypeRef::Json)),
            json!({"x": true}),
            r#"'{"x":true}'"#,
        ),
        (
            "vector",
            TypeRef::Vec(Box::new(TypeRef::Json)),
            json!([{"a": 1}, false]),
            r#"['{"a":1}', 'false']"#,
        ),
        (
            "map",
            TypeRef::Map(Box::new(TypeRef::String), Box::new(TypeRef::Json)),
            json!({"key": {"a": 1}}),
            r#"{ 'key' => '{"a":1}' }"#,
        ),
        ("ordinary_string", TypeRef::String, json!("hello"), "'hello'"),
        (
            "ordinary_map",
            TypeRef::Map(Box::new(TypeRef::String), Box::new(TypeRef::String)),
            json!({"key": "value"}),
            "{ 'key' => 'value' }",
        ),
    ]
}

fn render_case(ty: TypeRef, value: &Value) -> String {
    let types = vec![TypeDef {
        name: "Request".into(),
        fields: vec![field("payload", ty)],
        ..Default::default()
    }];
    super::values::typed_field_value(value, "payload", "Request", &types, "sample")
}

#[test]
fn ruby_json_field_literals_follow_the_magnus_string_contract() {
    let cases = cases();
    assert_eq!(cases.len(), 12);
    for (name, ty, value, expected) in cases {
        let actual = render_case(ty.clone(), &value);
        assert_eq!(actual, expected, "{name}");
        if matches!(ty, TypeRef::Json) {
            let typ = TypeDef {
                name: "Request".into(),
                rust_path: "sample_core::Request".into(),
                fields: vec![field("payload", ty)],
                has_default: true,
                ..Default::default()
            };
            let surface = ApiSurface {
                crate_name: "sample_core".into(),
                types: vec![typ],
                ..Default::default()
            };
            let generated = MagnusBackend
                .generate_bindings(&surface, &ResolvedCrateConfig::default())
                .unwrap();
            let constructor = &generated
                .iter()
                .find(|file| file.path.ends_with("lib.rs"))
                .unwrap()
                .content;
            assert!(constructor.contains("String::try_convert(v)"), "{constructor}");
        }
    }
}

fn fixture_and_config() -> (Fixture, E2eConfig, Vec<TypeDef>) {
    let fixture: Fixture = serde_json::from_value(json!({
        "id": "json_payload", "input": {"request": {"payload": {"limit": 7}, "nested": {"payload": [1, false]}}},
        "assertions": [{"type": "not_error"}]
    }))
    .unwrap();
    let mut config = E2eConfig::default();
    config.call.function = "process".into();
    config.call.module = "sample".into();
    config.call.args = vec![ArgMapping {
        name: "request".into(),
        field: "request".into(),
        arg_type: "json_object".into(),
        optional: false,
        owned: false,
        element_type: None,
        go_type: None,
        vec_inner_is_ref: false,
        trait_name: None,
    }];
    config.call.overrides.insert(
        "ruby".into(),
        CallOverride {
            options_type: Some("Request".into()),
            ..Default::default()
        },
    );
    let types = vec![
        TypeDef {
            name: "Request".into(),
            fields: vec![
                field("payload", TypeRef::Json),
                field("nested", TypeRef::Named("Nested".into())),
            ],
            ..Default::default()
        },
        TypeDef {
            name: "Nested".into(),
            fields: vec![field("payload", TypeRef::Json)],
            ..Default::default()
        },
    ];
    (fixture, config, types)
}

fn rendered_constructor() -> String {
    let (fixture, config, types) = fixture_and_config();
    super::args::build_args_and_setup(
        &fixture.input,
        &config.call.args,
        "sample",
        "sample",
        Some("Request"),
        &Default::default(),
        false,
        &fixture,
        None,
        &ResolvedCrateConfig::default(),
        &types,
    )
    .1
}

#[test]
fn ruby_snippet_and_spec_share_typed_json_constructor_values() {
    let (fixture, config, types) = fixture_and_config();
    let snippet =
        super::snippet::render_snippet_body(&fixture, &config, &ResolvedCrateConfig::default(), &types, &[]).unwrap();
    let spec = super::spec_file::render_spec_file(
        "configuration",
        &[&fixture],
        "sample",
        None,
        "sample",
        Some("Request"),
        &Default::default(),
        false,
        &config,
        false,
        false,
        &[],
        &ResolvedCrateConfig::default(),
        &types,
        &[],
        &[],
        &[],
    );
    let constructor = rendered_constructor();
    assert!(constructor.contains(r#"payload: '{"limit":7}'"#), "{constructor}");
    assert!(
        constructor.contains("Sample::Nested.new(payload: '[1,false]')"),
        "{constructor}"
    );
    assert!(snippet.contains(&constructor), "{snippet}");
    assert!(spec.contains(&constructor), "{spec}");
}

#[test]
fn ruby_executes_typed_json_literals_and_rejects_the_old_hash_shape() {
    let Some(ruby) = crate::test_support::toolchain::RUBY.open() else {
        return;
    };
    let cases: Vec<_> = cases().into_iter().map(|(name, ty, value, expected)| crate::alef_context! {
        name => crate::e2e::escape::ruby_string_literal(name), actual => render_case(ty, &value), expected => expected,
    }).collect();
    assert_eq!(cases.len(), 12);
    let program = crate::e2e::template_env::render(
        "ruby/json_field_runtime.rb.jinja",
        crate::alef_context! {
            cases => cases, count => 12, constructor => rendered_constructor(),
        },
    );
    assert_eq!(program.matches("raise '").count(), 17);
    let directory = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(ruby)
        .args(["-e", &program])
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{program}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "PASS: 12 JSON field cases, raw Hash negative control, and typed constructors"
    );
}

#[test]
fn optional_field_metadata_preserves_nil_and_wire_names_only_select_types() {
    let mut payload = field("json_payload", TypeRef::Json);
    payload.optional = true;
    payload.serde_rename = Some("wirePayload".into());
    let types = vec![TypeDef {
        name: "Request".into(),
        fields: vec![payload],
        ..Default::default()
    }];
    assert_eq!(
        super::values::typed_field_value(&Value::Null, "json_payload", "Request", &types, "sample"),
        "nil"
    );
    assert_eq!(
        super::values::typed_field_value(&json!({"x": 1}), "wirePayload", "Sample::Request", &types, "sample"),
        r#"'{"x":1}'"#
    );
}

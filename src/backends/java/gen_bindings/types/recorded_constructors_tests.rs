use super::render_recorded_constructors;
use crate::core::ir::{DefaultValue, FieldDef, PrimitiveType, TypeDef, TypeRef, VersionAnnotation};

fn options() -> TypeDef {
    TypeDef {
        name: "Options".into(),
        fields: ["enabled", "label", "extra"]
            .into_iter()
            .map(|name| FieldDef {
                name: name.into(),
                ty: TypeRef::Primitive(PrimitiveType::Bool),
                typed_default: Some(DefaultValue::BoolLiteral(false)),
                ..FieldDef::default()
            })
            .collect(),
        version: VersionAnnotation {
            legacy_constructor_fields: vec!["enabled".into()],
            constructor_history: vec![vec!["enabled".into(), "label".into()]],
            ..VersionAnnotation::default()
        },
        ..TypeDef::default()
    }
}

fn generate(owner: &TypeDef, declarations: &[&str]) -> anyhow::Result<String> {
    render_recorded_constructors(
        owner,
        &owner.fields.iter().collect::<Vec<_>>(),
        &declarations
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>(),
    )
}

#[test]
fn historical_arguments_are_forwarded_and_only_missing_fields_defaulted() {
    let output = generate(&options(), &["boolean enabled", "boolean label", "boolean extra"]).unwrap();
    assert!(output.contains("this(enabled, false, false);"), "{output}");
    assert!(output.contains("this(enabled, label, false);"), "{output}");
    assert_eq!(output.matches("public Options(").count(), 2);
}

#[test]
fn canonical_and_repeated_signatures_do_not_duplicate_constructors() {
    let mut owner = options();
    owner.version.constructor_history.push(vec!["enabled".into()]);
    owner
        .version
        .constructor_history
        .push(owner.fields.iter().map(|field| field.name.clone()).collect());
    let output = generate(&owner, &["boolean enabled", "boolean label", "boolean extra"]).unwrap();
    assert_eq!(output.matches("public Options(").count(), 2);
}

#[test]
fn malformed_and_incomplete_declarations_return_contextual_errors() {
    for declarations in [
        vec!["boolean", "String label", "boolean extra"],
        vec!["boolean enabled", "String", "boolean extra"],
        vec!["boolean enabled"],
    ] {
        let error = generate(&options(), &declarations).unwrap_err().to_string();
        assert!(error.contains("Options") && error.contains("declaration"), "{error}");
    }
}

#[test]
fn missing_fields_defaults_and_repeated_fields_are_rejected() {
    let mut owner = options();
    owner.version.constructor_history[0].push("hidden".into());
    assert!(
        generate(&owner, &["boolean enabled", "boolean label", "boolean extra"])
            .unwrap_err()
            .to_string()
            .contains("Options.hidden is not exposed")
    );
    owner.version.constructor_history[0].pop();
    owner.version.constructor_history[0].push("label".into());
    assert!(
        generate(&owner, &["boolean enabled", "boolean label", "boolean extra"])
            .unwrap_err()
            .to_string()
            .contains("repeats field label")
    );
    owner.version.constructor_history[0].pop();
    owner.fields[2].typed_default = None;
    let error = generate(&owner, &["boolean enabled", "boolean label", "boolean extra"]).unwrap_err();
    assert!(format!("{error:#}").contains("Options omits extra"));
    assert!(format!("{error:#}").contains("no resolved default"));
}

#[test]
fn java_erasure_collisions_between_distinct_field_sequences_are_rejected() {
    let mut owner = options();
    owner.version.legacy_constructor_fields = vec!["enabled".into()];
    owner.version.constructor_history = vec![vec!["label".into()]];
    let error = generate(
        &owner,
        &["List<String> enabled", "List<Integer> label", "boolean extra"],
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("Options") && error.contains("type erasure"), "{error}");
    owner.version.constructor_history = vec![vec!["label".into(), "enabled".into(), "extra".into()]];
    let error = generate(&owner, &["boolean enabled", "boolean label", "boolean extra"])
        .unwrap_err()
        .to_string();
    assert!(error.contains("type erasure"), "{error}");
}

#[test]
fn recorded_names_must_remain_visible_after_java_filtering() {
    let owner = options();
    let error = render_recorded_constructors(
        &owner,
        &[&owner.fields[0], &owner.fields[2]],
        &["boolean enabled".into(), "boolean extra".into()],
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("Options.label is not exposed"), "{error}");
}

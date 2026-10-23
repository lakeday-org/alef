use crate::core::ir::{EnumDef, EnumVariant, FieldDef, TypeDef, TypeRef};
use crate::e2e::field_access::FieldResolver;
use crate::e2e::fixture::Assertion;

fn field(name: &str, ty: TypeRef) -> FieldDef {
    FieldDef {
        name: name.into(),
        optional: matches!(ty, TypeRef::Optional(_)),
        ty,
        ..Default::default()
    }
}

fn types() -> Vec<TypeDef> {
    vec![
        TypeDef {
            name: "Envelope".into(),
            fields: vec![field("entries", TypeRef::Vec(Box::new(TypeRef::Named("Entry".into()))))],
            ..Default::default()
        },
        TypeDef {
            name: "Entry".into(),
            fields: vec![field("body", TypeRef::Named("Body".into()))],
            ..Default::default()
        },
        TypeDef {
            name: "Body".into(),
            fields: vec![field(
                "payload",
                TypeRef::Optional(Box::new(TypeRef::Named("Payload".into()))),
            )],
            ..Default::default()
        },
        TypeDef {
            name: "Piece".into(),
            fields: vec![field("text", TypeRef::String)],
            ..Default::default()
        },
    ]
}

fn payload_enum() -> EnumDef {
    EnumDef {
        name: "Payload".into(),
        serde_untagged: true,
        has_serde: true,
        variants: vec![
            EnumVariant {
                name: "Segments".into(),
                is_tuple: true,
                fields: vec![field("_0", TypeRef::Vec(Box::new(TypeRef::Named("Piece".into()))))],
                ..Default::default()
            },
            EnumVariant {
                name: "Scalar".into(),
                is_tuple: true,
                fields: vec![field("_0", TypeRef::String)],
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

pub(crate) fn api(enum_elements: bool, wire_rename: bool) -> crate::core::ir::ApiSurface {
    let mut types = types();
    let mut enums = vec![payload_enum()];
    if enum_elements {
        types.retain(|ty| ty.name != "Piece");
        let mut text = field("text", TypeRef::String);
        if wire_rename {
            text.serde_rename = Some("payload_text".into());
        }
        enums.push(EnumDef {
            name: "Piece".into(),
            serde_tag: Some("kind".into()),
            has_serde: true,
            variants: vec![EnumVariant {
                name: "Text".into(),
                fields: vec![text],
                ..Default::default()
            }],
            ..Default::default()
        });
    }
    for ty in &mut types {
        ty.rust_path = format!("sample::{}", ty.name);
    }
    for en in &mut enums {
        en.rust_path = format!("sample::{}", en.name);
    }
    crate::core::ir::ApiSurface {
        version: "1.0.0".into(),
        crate_name: "sample".into(),
        types,
        enums,
        ..Default::default()
    }
}

pub(crate) fn resolver(language: &str, enum_elements: bool, wire_rename: bool) -> FieldResolver {
    let api = api(enum_elements, wire_rename);
    let types = &api.types;
    let enums = &api.enums;
    let root = Some("Envelope".into());
    let (reachable, excluded, optional) = FieldResolver::ir_field_sets(types);
    FieldResolver::new(
        &Default::default(),
        &Default::default(),
        &Default::default(),
        &Default::default(),
        &Default::default(),
    )
    .with_ir_enum_map(FieldResolver::ir_enum_fields(types, enums), root.clone())
    .with_ir_collection_map(FieldResolver::ir_collection_fields(types), root.clone())
    .with_ir_result_fields(FieldResolver::ir_result_field_facts(types, language), root)
    .with_ir_fields(reachable, excluded, optional)
}

pub(crate) fn assertions(wire_rename: bool) -> Vec<Assertion> {
    [
        ("count_equals", "entries[0].body.payload.segments", serde_json::json!(2)),
        (
            "equals",
            "entries[0].body.payload.segments[0].text",
            serde_json::json!("hello"),
        ),
        (
            "equals",
            "entries[0].body.payload.segments[1].text",
            serde_json::json!(" world"),
        ),
    ]
    .into_iter()
    .map(|(kind, field, value)| Assertion {
        assertion_type: kind.into(),
        field: Some(if wire_rename {
            field.replace("].text", "].payload_text")
        } else {
            field.into()
        }),
        value: Some(value),
        ..Default::default()
    })
    .collect()
}

pub(crate) fn verify(source: &str, count_expression: &str) {
    assert!(
        source.contains(count_expression),
        "missing executable count assertion:\n{source}"
    );
    for expected in ["hello", " world"] {
        assert!(
            source.contains(expected),
            "missing indexed value assertion {expected:?}:\n{source}"
        );
    }
    assert!(
        !source.contains("skipped:"),
        "supported assertions were dropped:\n{source}"
    );
}

use super::*;
use crate::core::ir::{EnumDef, EnumVariant, FieldDef, TypeDef, TypeRef};
use std::collections::{HashMap, HashSet};

fn field(name: &str, ty: TypeRef) -> FieldDef {
    FieldDef {
        name: name.into(),
        ty,
        ..Default::default()
    }
}

fn named(name: &str) -> TypeRef {
    TypeRef::Named(name.into())
}

fn resolver(root: &str) -> FieldResolver {
    let definitions = [
        (
            "Envelope",
            vec![field("entries", TypeRef::Vec(Box::new(named("Entry"))))],
        ),
        ("Entry", vec![field("body", named("Body"))]),
        ("Body", vec![field("content", named("Payload"))]),
        (
            "Piece",
            vec![field("text", TypeRef::String), field("details", named("Details"))],
        ),
        ("Details", vec![field("name", TypeRef::String)]),
    ];
    let types: Vec<_> = definitions
        .into_iter()
        .map(|(name, fields)| TypeDef {
            name: name.into(),
            fields,
            ..Default::default()
        })
        .collect();
    let enums = vec![EnumDef {
        name: "Payload".into(),
        variants: payload_variants(),
        ..Default::default()
    }];
    FieldResolver::new(
        &HashMap::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
    )
    .with_ir_enum_map(FieldResolver::ir_enum_fields(&types, &enums), Some(root.into()))
}

fn payload_variants() -> Vec<EnumVariant> {
    let matrix = TypeRef::Vec(Box::new(TypeRef::Vec(Box::new(named("Piece")))));
    [
        ("Segments", TypeRef::Vec(Box::new(named("Piece")))),
        ("Matrix", matrix),
        ("Object", named("Piece")),
    ]
    .into_iter()
    .map(|(name, ty)| EnumVariant {
        name: name.into(),
        fields: vec![field("0", ty)],
        is_tuple: true,
        ..Default::default()
    })
    .collect()
}

type SplitCase = (&'static str, &'static str, &'static str, &'static str, &'static str);

fn indexed_variant_cases() -> [SplitCase; 9] {
    [
        ("Payload", "segments", "", "Segments", ""),
        ("Payload", "segments[0].text", "", "Segments", "[0].text"),
        (
            "Payload",
            "segments[1].details.name",
            "",
            "Segments",
            "[1].details.name",
        ),
        ("Payload", "matrix[0][1].text", "", "Matrix", "[0][1].text"),
        ("Payload", "object.text", "", "Object", "text"),
        (
            "Envelope",
            "entries[0].body.content.segments",
            "entries[0].body.content",
            "Segments",
            "",
        ),
        (
            "Envelope",
            "entries[0].body.content.segments[0].text",
            "entries[0].body.content",
            "Segments",
            "[0].text",
        ),
        (
            "Envelope",
            "entries[1].body.content.matrix[0][1].text",
            "entries[1].body.content",
            "Matrix",
            "[0][1].text",
        ),
        (
            "Envelope",
            "entries[0].body.content.object.text",
            "entries[0].body.content",
            "Object",
            "text",
        ),
    ]
}

#[test]
fn indexed_variant_split_retains_payload_indices_and_leaf_paths() {
    let cases = indexed_variant_cases();
    assert_eq!(cases.len(), 9);
    for (root, path, prefix, variant, suffix) in cases {
        let resolver = resolver(root);
        let expected = Some((prefix.into(), "Payload".into(), variant.into(), suffix.into()));
        assert_eq!(
            resolver.ir_tagged_union_split(path),
            expected,
            "indexed variant path {path}"
        );
        assert_eq!(
            resolver.ir_tagged_union_split_from(root, path),
            expected,
            "owner-relative path {path}"
        );
    }
}

#[test]
fn indexed_variant_split_preserves_non_union_and_unknown_variant_refusals() {
    let root = resolver("Payload");
    for path in ["missing.text", "segments[key].text", "segments[-1].text"] {
        assert_eq!(root.ir_tagged_union_split(path), None, "{path}");
    }
    let nested = resolver("Envelope");
    assert_eq!(nested.ir_tagged_union_split("entries[0].missing.text"), None);
    assert_eq!(nested.ir_tagged_union_split("entries[0].body.content"), None);
    assert_eq!(
        nested.ir_tagged_union_split("entries[0].body.content.unknown.text"),
        Some((
            "entries[0].body.content".into(),
            "Payload".into(),
            "Unknown".into(),
            "text".into()
        ))
    );
}

use crate::core::ir::{EnumDef, EnumVariant, FieldDef, PrimitiveType, TypeRef};
use std::collections::HashSet;

fn variant(name: &str, ty: TypeRef) -> EnumVariant {
    EnumVariant {
        name: name.into(),
        is_tuple: true,
        fields: vec![FieldDef {
            name: "_0".into(),
            ty,
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn scalar_payload_types_qualify_only_builtin_variant_collisions() {
    use super::super::super::types::kotlin_type_disambiguated;
    let cases = [
        ("Boolean", PrimitiveType::Bool),
        ("Byte", PrimitiveType::I8),
        ("Byte", PrimitiveType::U8),
        ("Short", PrimitiveType::I16),
        ("Short", PrimitiveType::U16),
        ("Int", PrimitiveType::I32),
        ("Int", PrimitiveType::U32),
        ("Long", PrimitiveType::I64),
        ("Long", PrimitiveType::U64),
        ("Long", PrimitiveType::Isize),
        ("Long", PrimitiveType::Usize),
        ("Float", PrimitiveType::F32),
        ("Double", PrimitiveType::F64),
    ];
    for (name, primitive) in cases {
        let ty = TypeRef::Primitive(primitive);
        let collision = HashSet::from([name]);
        let other = HashSet::from(["Other"]);
        assert_eq!(
            kotlin_type_disambiguated(&ty, false, &collision, "sample"),
            format!("kotlin.{name}")
        );
        assert_eq!(kotlin_type_disambiguated(&ty, false, &other, "sample"), name);
    }
}

#[test]
fn scalar_shadowing_is_disambiguated_recursively_and_named_types_keep_their_package() {
    use super::super::super::types::kotlin_type_disambiguated;
    let names = HashSet::from(["Boolean", "String", "List", "Map"]);
    for (ty, expected) in [
        (TypeRef::String, "kotlin.String"),
        (TypeRef::Named("Boolean".into()), "sample.Boolean"),
        (
            TypeRef::Optional(Box::new(TypeRef::Primitive(PrimitiveType::Bool))),
            "kotlin.Boolean?",
        ),
        (
            TypeRef::Vec(Box::new(TypeRef::Primitive(PrimitiveType::Bool))),
            "kotlin.collections.List<kotlin.Boolean>",
        ),
        (
            TypeRef::Map(
                Box::new(TypeRef::String),
                Box::new(TypeRef::Primitive(PrimitiveType::Bool)),
            ),
            "kotlin.collections.Map<kotlin.String, kotlin.Boolean>",
        ),
    ] {
        assert_eq!(kotlin_type_disambiguated(&ty, false, &names, "sample"), expected);
    }
}

#[test]
fn untagged_boolean_shape_is_distinct_and_numeric_dispatch_order_is_preserved() {
    let en = EnumDef {
        name: "ScalarValue".into(),
        serde_untagged: true,
        variants: vec![
            variant("Count", TypeRef::Primitive(PrimitiveType::U64)),
            variant("Signed", TypeRef::Primitive(PrimitiveType::I64)),
            variant("Number", TypeRef::Primitive(PrimitiveType::F64)),
            variant("Boolean", TypeRef::Primitive(PrimitiveType::Bool)),
        ],
        ..Default::default()
    };
    let mut output = String::new();
    super::super::emit_enum(&en, &mut output, "sample", &[]);
    assert!(
        output.contains("data class Boolean(val value: kotlin.Boolean)"),
        "{output}"
    );
    let mut previous = 0;
    for (name, kind, builtin) in [
        ("Count", "Number", "Long"),
        ("Signed", "Number", "Long"),
        ("Number", "Number", "Double"),
        ("Boolean", "Boolean", "Boolean"),
    ] {
        let arm = format!(
            "if (node.is{kind}) return ScalarValue.{name}(ctx.readTreeAsValue(node, kotlin.{builtin}::class.java))"
        );
        let position = output.find(&arm).unwrap_or_else(|| panic!("missing {arm}: {output}"));
        assert!(position > previous, "variant order must stay unchanged: {output}");
        previous = position;
    }
    assert_eq!(output.matches("if (node.isNumber)").count(), 3);
    assert_eq!(output.matches("if (node.isBoolean)").count(), 1);
}

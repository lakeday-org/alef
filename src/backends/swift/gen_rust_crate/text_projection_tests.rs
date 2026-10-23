use super::*;
use crate::core::ir::{NewtypeWrapper, NewtypeWrapperMetadata};

#[test]
fn configured_text_projection_has_one_return_shape_authority() {
    let configured = HashSet::from(["BodyValue"]);
    for (ty, optional, expected) in [
        (TypeRef::Named("BodyValue".into()), false, Some("String")),
        (TypeRef::Named("BodyValue".into()), true, Some("Option<String>")),
        (
            TypeRef::Optional(Box::new(TypeRef::Named("BodyValue".into()))),
            false,
            Some("Option<String>"),
        ),
        (TypeRef::Named("OtherValue".into()), true, None),
        (TypeRef::String, false, None),
    ] {
        let field = FieldDef {
            ty,
            optional,
            ..Default::default()
        };
        assert_eq!(text_projected_getter_type(&field, &configured), expected);
        assert_eq!(text_projected_getter_type(&field, &HashSet::new()), None);
    }
}

#[test]
fn explicit_newtype_conversion_keeps_its_existing_transport_precedence() {
    let metadata = NewtypeWrapperMetadata::transparent_string(
        "sample::BodyValue",
        "sample::BodyValue::from",
        "sample::BodyValue::into_inner",
        vec![],
    );
    let field = FieldDef {
        ty: TypeRef::Named("BodyValue".into()),
        newtype_wrapper: Some(NewtypeWrapper::encode_explicit(&[metadata])),
        ..Default::default()
    };
    assert_eq!(text_projected_getter_type(&field, &HashSet::from(["BodyValue"])), None);
}

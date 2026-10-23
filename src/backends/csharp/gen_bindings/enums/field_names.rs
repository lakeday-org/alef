use crate::backends::csharp::type_map::csharp_type;
use crate::codegen::naming::{csharp_variant_name, to_csharp_name};
use crate::core::ir::{EnumDef, EnumVariant, FieldDef};

pub(crate) fn variant_field_type(enum_def: &EnumDef, field: &FieldDef) -> String {
    let ty = if field.sanitized && field.type_rust_path.is_some() {
        "object".to_string()
    } else {
        csharp_type(&field.ty).to_string()
    };
    let ty = if field.optional && !ty.ends_with('?') {
        format!("{ty}?")
    } else {
        ty
    };
    if enum_def
        .variants
        .iter()
        .any(|variant| ty.starts_with(&format!("{}<", csharp_variant_name(&variant.name))))
    {
        ty.replace("List<", "global::System.Collections.Generic.List<")
            .replace("Dictionary<", "global::System.Collections.Generic.Dictionary<")
    } else {
        ty
    }
}

pub(crate) fn variant_field_name(enum_def: &EnumDef, variant: &EnumVariant, field: &FieldDef) -> String {
    let public = to_csharp_name(field.name.trim_start_matches('_'));
    let clashes = public == csharp_variant_name(&variant.name)
        || public == variant_field_type(enum_def, field)
        || enum_def
            .variants
            .iter()
            .any(|other| csharp_variant_name(&other.name) == public);
    if super::super::is_tuple_field(field) || clashes {
        "Value".to_string()
    } else {
        public
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ir::TypeRef;

    #[test]
    fn collision_policy_matches_released_properties_without_using_wire_names() {
        let mut enum_def = EnumDef {
            variants: vec![
                EnumVariant {
                    name: "Text".into(),
                    ..Default::default()
                },
                EnumVariant {
                    name: "Other".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        for (name, ty, expected) in [
            ("text", TypeRef::String, "Value"),
            ("other", TypeRef::String, "Value"),
            ("details", TypeRef::Named("Details".into()), "Value"),
            ("caption", TypeRef::String, "Caption"),
            ("_0", TypeRef::String, "Value"),
        ] {
            let field = FieldDef {
                name: name.into(),
                ty,
                serde_rename: Some("wire_only".into()),
                ..Default::default()
            };
            enum_def.variants[0].fields = vec![field.clone()];
            assert_eq!(variant_field_name(&enum_def, &enum_def.variants[0], &field), expected);
        }
    }
}

use std::collections::HashMap;

use crate::codegen::naming::{wire_field_name, wire_variant_value};
use crate::codegen::serde_enum_repr::{SerdeEnumRepr, serde_enum_repr};
use crate::core::ir::{EnumDef, EnumVariant, TypeDef};

pub(super) fn struct_fields(types: &[TypeDef]) -> HashMap<String, HashMap<String, String>> {
    types
        .iter()
        .map(|typ| {
            let fields = typ
                .fields
                .iter()
                .map(|field| {
                    (
                        field.name.clone(),
                        wire_field_name(
                            &field.name,
                            field.serde_rename.as_deref(),
                            typ.serde_rename_all.as_deref(),
                        ),
                    )
                })
                .collect();
            (typ.name.clone(), fields)
        })
        .collect()
}

pub(super) fn variant_prefix(enum_def: &EnumDef, variant: &EnumVariant) -> Vec<String> {
    if variant.serde_untagged {
        return Vec::new();
    }
    match serde_enum_repr(enum_def) {
        SerdeEnumRepr::External => vec![wire_variant_value(
            &variant.name,
            variant.serde_rename.as_deref(),
            enum_def.serde_rename_all.as_deref(),
        )],
        SerdeEnumRepr::Adjacent { content, .. } => vec![content],
        SerdeEnumRepr::Internal { .. } | SerdeEnumRepr::Untagged => Vec::new(),
    }
}

pub(super) fn variant_fields(enum_def: &EnumDef, variant: &EnumVariant) -> HashMap<String, String> {
    variant
        .fields
        .iter()
        .map(|field| {
            (
                field.name.clone(),
                wire_field_name(
                    &field.name,
                    field.serde_rename.as_deref(),
                    enum_def.rename_all_fields.as_deref(),
                ),
            )
        })
        .collect()
}

pub(super) fn value_kind(ty: &crate::core::ir::TypeRef) -> super::types::WireValueKind {
    use super::types::WireValueKind;
    use crate::core::ir::TypeRef;
    match ty {
        TypeRef::String => WireValueKind::String,
        TypeRef::Vec(_) => WireValueKind::Collection,
        _ => WireValueKind::Unknown,
    }
}

pub(super) fn struct_kinds(types: &[TypeDef]) -> HashMap<String, HashMap<String, super::types::WireValueKind>> {
    types
        .iter()
        .map(|typ| {
            (
                typ.name.clone(),
                typ.fields
                    .iter()
                    .map(|field| (field.name.clone(), value_kind(&field.ty)))
                    .collect(),
            )
        })
        .collect()
}

pub(super) fn variant_kinds(variant: &EnumVariant) -> HashMap<String, super::types::WireValueKind> {
    variant
        .fields
        .iter()
        .map(|field| (field.name.clone(), value_kind(&field.ty)))
        .collect()
}

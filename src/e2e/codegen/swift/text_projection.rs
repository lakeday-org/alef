use crate::core::ir::{EnumDef, TypeDef, TypeRef};
use crate::e2e::field_access::SwiftFirstClassMap;
use std::collections::HashSet;

pub(super) fn normalized_fields(types: &[TypeDef]) -> Vec<TypeDef> {
    types
        .iter()
        .cloned()
        .map(|mut definition| {
            for field in &mut definition.fields {
                field.optional |= matches!(field.ty, TypeRef::Optional(_));
            }
            definition
        })
        .collect()
}

pub(super) fn apply(mut map: SwiftFirstClassMap, types: &[TypeDef], configured: &[String]) -> SwiftFirstClassMap {
    let text_types: HashSet<&str> = configured.iter().map(String::as_str).collect();
    for definition in types {
        for field in &definition.fields {
            let Some(return_type) =
                crate::backends::swift::gen_rust_crate::type_bridge::text_projected_getter_type(field, &text_types)
            else {
                continue;
            };
            map.getter_optionality
                .entry(definition.name.clone())
                .or_default()
                .insert(field.name.clone(), return_type == "Option<String>");
            map.json_bridged_by_type
                .entry(definition.name.clone())
                .or_default()
                .insert(field.name.clone(), false);
        }
    }
    map.json_bridged_field_names = map
        .json_bridged_by_type
        .values()
        .flat_map(|fields| {
            fields
                .iter()
                .filter_map(|(name, bridged)| bridged.then_some(name.clone()))
        })
        .collect();
    map
}

pub(super) fn lossless_fields(
    types: &[TypeDef],
    enums: &[EnumDef],
    configured: &[String],
) -> std::collections::HashMap<String, HashSet<String>> {
    let serde_text_types = enums
        .iter()
        .filter(|definition| definition.has_serde && configured.contains(&definition.name))
        .map(|definition| definition.name.as_str())
        .collect();
    let mut fields = std::collections::HashMap::<String, HashSet<String>>::new();
    for definition in types {
        for field in &definition.fields {
            if definition.has_serde
                && !field.serde_skip
                && crate::backends::swift::gen_rust_crate::type_bridge::text_projected_getter_type(
                    field,
                    &serde_text_types,
                )
                .is_some()
            {
                fields
                    .entry(definition.name.clone())
                    .or_default()
                    .insert(field.name.clone());
            }
        }
    }
    fields
}

pub(super) fn resolver(
    resolver: crate::e2e::field_access::FieldResolver,
    types: &[TypeDef],
    enums: &[EnumDef],
    config: &crate::core::config::ResolvedCrateConfig,
) -> crate::e2e::field_access::FieldResolver {
    resolver.with_swift_lossless_text_fields(lossless_fields(types, enums, &config.untagged_union_text_types))
}

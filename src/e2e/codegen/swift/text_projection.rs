use crate::core::ir::{TypeDef, TypeRef};
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

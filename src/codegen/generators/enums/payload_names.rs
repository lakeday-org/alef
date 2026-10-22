use std::collections::HashSet;

use super::collect_pyo3_variant_constructors;
use crate::codegen::naming::python_enum_payload_getter_name;
use crate::core::ir::{EnumDef, EnumVariant};

/// Reserve every factory and bare getter before allocating payload getter suffixes. ~keep
pub(crate) fn dict_getter_py_name(enum_def: &EnumDef, variant: &EnumVariant) -> String {
    let constructors = collect_pyo3_variant_constructors(enum_def);
    let mut fixed: HashSet<String> = constructors.iter().map(|ctor| ctor.snake_name.clone()).collect();
    if let Some(tag) = &enum_def.serde_tag {
        fixed.insert(tag.clone());
    }
    let bare_constructors: HashSet<&str> = constructors
        .iter()
        .filter(|ctor| !ctor.is_tuple)
        .map(|ctor| ctor.variant_name)
        .collect();
    let empty = HashSet::new();
    let mut reserved = fixed.clone();
    reserved.extend(
        enum_def
            .variants
            .iter()
            .map(|item| python_enum_payload_getter_name(&item.name, false, &empty)),
    );
    let mut allocated = HashSet::new();
    for item in &enum_def.variants {
        let bare = python_enum_payload_getter_name(&item.name, false, &empty);
        if !fixed.contains(&bare) && !allocated.contains(&bare) {
            reserved.remove(&bare);
        }
        let name =
            python_enum_payload_getter_name(&item.name, bare_constructors.contains(item.name.as_str()), &reserved);
        reserved.insert(name.clone());
        allocated.insert(name.clone());
        if item.name == variant.name {
            return name;
        }
    }
    python_enum_payload_getter_name(&variant.name, false, &reserved)
}

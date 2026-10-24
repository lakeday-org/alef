use crate::core::ir::{FieldDef, TypeRef};
use std::collections::HashSet;

pub(crate) fn json_getter_name(field: &str) -> String {
    crate::codegen::naming::internal_rust_identifier(&format!(
        "__alef_json_{}",
        crate::codegen::naming::pascal_to_snake(field)
    ))
}

pub(super) fn emit_extern(field: &FieldDef, configured: &HashSet<&str>, out: &mut String) {
    if super::type_bridge::text_projected_getter_type(field, configured).is_none() {
        return;
    }
    out.push_str(&crate::backends::swift::template_env::render(
        "extern_fn_getter.jinja",
        crate::alef_context! { name => json_getter_name(&field.name), return_type => super::bridge_error::result_type("String") },
    ));
}

pub(super) fn emit_getters(
    field: &FieldDef,
    getter: &str,
    name: &str,
    return_type: &str,
    lossless: bool,
    out: &mut String,
) {
    out.push_str(&crate::backends::swift::template_env::render(
        "getter_display_text.rs.jinja",
        crate::alef_context! { getter_name => getter, return_type => return_type, name => name,
        optional => field.optional || matches!(field.ty, TypeRef::Optional(_)) },
    ));
    if !lossless {
        return;
    }
    out.push_str(&crate::backends::swift::template_env::render(
        "getter_lossless_json.rs.jinja",
        crate::alef_context! { getter_name => json_getter_name(&field.name), name => name,
        return_type => super::bridge_error::result_type("String") },
    ));
}

pub(crate) fn serde_text_types<'a>(api: &'a crate::core::ir::ApiSurface, configured: &[String]) -> HashSet<&'a str> {
    api.enums
        .iter()
        .filter(|definition| definition.has_serde && configured.contains(&definition.name))
        .map(|definition| definition.name.as_str())
        .collect()
}

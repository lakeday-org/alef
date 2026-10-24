use crate::core::ir::{EnumVariant, PrimitiveType};

pub(super) fn emit_deserialize_arm(out: &mut String, name: &str, variant: &EnumVariant, primitive: &PrimitiveType) {
    let condition = match primitive {
        PrimitiveType::Bool => "node.isBoolean",
        _ => "node.isNumber",
    };
    let builtin = super::super::types::primitive_type_name(primitive);
    out.push_str(&crate::backends::kotlin::template_env::render(
        "untagged_primitive_variant.jinja",
        crate::alef_context! {
            condition => condition,
            enum_name => name,
            variant_name => &variant.name,
            builtin => builtin,
        },
    ));
}

#[cfg(test)]
mod tests;

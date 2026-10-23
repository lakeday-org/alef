use super::{ErrorDef, wasm_payload_type_is_safe};

#[derive(serde::Serialize)]
pub(super) struct PayloadField<'a> {
    name: &'a str,
    binding: String,
    value: String,
    expose_flat: bool,
}

fn reserved_property(name: &str, properties: &[(String, String)]) -> bool {
    matches!(
        name,
        "message"
            | "name"
            | "stack"
            | "cause"
            | "code"
            | "kind"
            | "errorCode"
            | "payload"
            | "__proto__"
            | "constructor"
            | "toString"
            | "toLocaleString"
            | "valueOf"
            | "hasOwnProperty"
            | "isPrototypeOf"
            | "propertyIsEnumerable"
    ) || properties.iter().any(|(property, _)| property == name)
}

pub(super) fn variants<'a>(
    error: &'a ErrorDef,
    rust_path: &str,
    properties: &[(String, String)],
) -> Vec<(String, Vec<PayloadField<'a>>)> {
    error
        .variants
        .iter()
        .filter(|variant| !variant.is_unit && !variant.is_tuple)
        .filter_map(|variant| {
            let fields: Vec<_> = variant
                .fields
                .iter()
                .filter(|field| {
                    !field.binding_excluded
                        && !field.serde_skip
                        && field.cfg.is_none()
                        && !field.sanitized
                        && !field.name.starts_with('_')
                        && wasm_payload_type_is_safe(&field.ty)
                })
                .enumerate()
                .map(|(index, field)| PayloadField {
                    name: &field.name,
                    binding: crate::codegen::naming::internal_rust_identifier(&format!("__alef_error_field_{index}")),
                    value: crate::codegen::naming::internal_rust_identifier(&format!("__alef_error_value_{index}")),
                    expose_flat: !reserved_property(&field.name, properties),
                })
                .collect();
            if fields.is_empty() {
                return None;
            }
            let names = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: {}",
                        crate::codegen::naming::internal_rust_identifier(field.name),
                        field.binding
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            Some((format!("{rust_path}::{} {{ {names}, .. }}", variant.name), fields))
        })
        .collect()
}

#[cfg(test)]
#[path = "wasm_payload/tests.rs"]
mod tests;

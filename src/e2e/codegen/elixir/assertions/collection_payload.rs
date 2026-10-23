use crate::e2e::field_access::{FieldResolver, WireAccessStep};
use crate::e2e::fixture::Assertion;

/// Returns true if the field expression is a numeric/integer expression
/// (e.g., a `length(...)` call) rather than a string.
pub(super) fn is_numeric_expr(field_expr: &str) -> bool {
    field_expr.starts_with("length(")
}

/// Build a call to the generated enum module's `wire_value/1` function.
///
/// Enum submodules (`AppModule.EnumName`) live under the app's top-level Elixir module,
/// regardless of which (possibly nested) module a given NIF call is dispatched through --
/// see `enum_module_header.jinja` in the rustler backend, which always nests enum modules one
/// level under `app_module`. `module_path` here is the *call's* module (`E2eConfig`'s
/// `[crates.e2e.call] module`, e.g. `MyLib` or a nested `MyLib.Service`), so only its first
/// dot-segment is the app root the enum module actually lives under. ~keep
pub(super) fn elixir_enum_wire_value_expr(module_path: &str, enum_type_name: &str, field_expr: &str) -> String {
    let app_root = module_path.split('.').next().unwrap_or(module_path);
    format!("{app_root}.{enum_type_name}.wire_value({field_expr})")
}

pub(super) fn render(out: &mut String, assertion: &Assertion, result: &str, resolver: &FieldResolver) -> bool {
    let Some(field) = assertion.field.as_deref() else {
        return false;
    };
    let Some((prefix, owner, variant, suffix)) = resolver.ir_tagged_union_split(resolver.resolve(field)) else {
        return false;
    };
    if !resolver.union_variant_payload_is_collection(&owner, &variant) {
        return false;
    }
    let expression = expression(resolver, &prefix, &owner, &variant, &suffix, result);
    out.push_str(&crate::e2e::template_env::render(
        "elixir/collection_payload.jinja",
        crate::alef_context! {
            expression => expression, assertion_type => if expression.is_some() { assertion.assertion_type.as_str() } else { "unsupported" },
            expected => assertion.value.as_ref().map(super::json_to_elixir), field => field,
        },
    ));
    true
}

fn expression(
    resolver: &FieldResolver,
    prefix: &str,
    owner: &str,
    variant: &str,
    suffix: &str,
    result: &str,
) -> Option<String> {
    let definition = resolver.enum_definition(owner)?;
    if !crate::backends::rustler::gen_bindings::helpers::is_flat_data_enum(definition) {
        return None;
    }
    let name = crate::codegen::naming::pascal_to_snake(variant);
    let container = resolver.accessor(prefix, "elixir", result);
    let mut expression = format!("{container}.{name}");
    let mut owner = resolver.union_variant_payload(owner, variant)?.1.to_string();
    for step in resolver.enum_payload_wire_steps(&definition.name, variant, suffix)? {
        match step {
            WireAccessStep::Index(index) => expression = format!("Enum.at({expression}, {index})"),
            WireAccessStep::Field(key) => {
                let metadata = resolver.enum_field_metadata(&owner, &key)?;
                let definition = resolver.enum_definition(&owner)?;
                if crate::backends::rustler::gen_bindings::helpers::is_flat_data_enum(definition) {
                    return None;
                }
                let tag = crate::backends::rustler::elixir_atom_body(&crate::codegen::naming::pascal_to_snake(
                    &metadata.variant,
                ));
                let name = crate::backends::rustler::elixir_atom_body(&crate::codegen::naming::to_elixir_name(
                    &metadata.definition.name,
                ));
                expression = crate::e2e::template_env::render(
                    "elixir/tagged_payload_field.jinja",
                    crate::alef_context! { expression => expression, tag => tag, name => name },
                )
                .trim()
                .to_string();
                owner = metadata.next_type.unwrap_or_default();
            }
        }
    }
    Some(expression)
}

#[cfg(test)]
#[path = "collection_payload/tests.rs"]
mod tests;

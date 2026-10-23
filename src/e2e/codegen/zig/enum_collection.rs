use crate::e2e::field_access::{FieldResolver, WireAccessStep};

pub(super) fn accessor(result: &str, field: &str, resolver: &FieldResolver) -> Option<String> {
    let (prefix, owner, variant, suffix) = resolver.ir_tagged_union_split(resolver.resolve(field))?;
    let definition = resolver.enum_definition(&owner)?;
    if !definition.serde_untagged
        || !definition
            .variants
            .iter()
            .any(|item| item.name == variant && item.is_tuple)
        || !resolver.union_variant_payload_is_collection(&owner, &variant)
    {
        return None;
    }
    let steps = resolver.enum_payload_wire_steps(&owner, &variant, &suffix)?;
    let steps = steps
        .into_iter()
        .map(|step| match step {
            WireAccessStep::Index(index) => crate::alef_context! { kind => "index", index => index },
            WireAccessStep::Field(key) => {
                crate::alef_context! { kind => "field", key => crate::e2e::escape::escape_zig(&key) }
            }
        })
        .collect::<Vec<_>>();
    let expression =
        super::json_access::json_path_expr_with(result, "", &prefix, super::json_access::Unwrap::Panic, resolver);
    Some(
        crate::e2e::template_env::render(
            "zig/enum_collection_accessor.jinja",
            crate::alef_context! {
                expression => expression, steps => steps,
            },
        )
        .trim_end()
        .to_string(),
    )
}

#[cfg(test)]
#[path = "enum_collection/tests.rs"]
mod tests;

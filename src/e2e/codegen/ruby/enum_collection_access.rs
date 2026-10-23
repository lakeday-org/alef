use crate::e2e::field_access::{FieldResolver, WireAccessStep};

pub(super) fn accessor(resolver: &FieldResolver, field: &str, result: &str) -> Option<String> {
    let (prefix, enum_type, variant, suffix) = resolver.ir_tagged_union_split(resolver.resolve(field))?;
    if !resolver.union_variant_payload_is_collection(&enum_type, &variant) {
        return None;
    }
    let definition = resolver.enum_definition(&enum_type)?;
    if !definition.serde_untagged || !definition.variants.iter().find(|item| item.name == variant)?.is_tuple {
        return None;
    }
    let steps = resolver.enum_payload_wire_steps(&enum_type, &variant, &suffix)?;
    let steps = steps
        .into_iter()
        .map(|step| match step {
            WireAccessStep::Index(index) => index.to_string(),
            WireAccessStep::Field(key) => format!("{}.to_sym", crate::e2e::escape::ruby_string_literal(&key)),
        })
        .collect::<Vec<_>>();
    let expression = resolver.accessor(&prefix, "ruby", result);
    Some(
        crate::e2e::template_env::render(
            "ruby/enum_collection_accessor.jinja",
            crate::alef_context! { enum_expression => expression, steps => steps },
        )
        .trim_end()
        .to_string(),
    )
}

#[cfg(test)]
#[path = "enum_collection_access/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "enum_collection_access/producer_tests.rs"]
mod producer_tests;

use crate::e2e::field_access::WireAccessStep;
use crate::e2e::fixture::Assertion;

use super::AssertionRenderContext;

pub(super) fn try_render(out: &mut String, context: &AssertionRenderContext<'_>, assertion: &Assertion) -> bool {
    let Some(field) = assertion.field.as_deref() else {
        return false;
    };
    let Some(value) = assertion.value.as_ref() else {
        return false;
    };
    if !matches!(
        assertion.assertion_type.as_str(),
        "equals" | "count_equals" | "count_min"
    ) {
        return false;
    }
    if assertion.assertion_type == "equals" && !value.is_string()
        || assertion.assertion_type != "equals" && !value.is_u64()
    {
        return false;
    }
    let resolver = context.field_resolver;
    let Some((prefix, owner, variant, suffix)) = resolver.ir_tagged_union_split(resolver.resolve(field)) else {
        return false;
    };
    let Some(definition) = resolver.enum_definition(&owner) else {
        return false;
    };
    if !crate::backends::go::is_passthrough_raw_message_enum(definition)
        || !resolver.union_variant_payload_is_collection(&owner, &variant)
    {
        return false;
    }
    let Some(steps) = resolver.enum_payload_wire_steps(&owner, &variant, &suffix) else {
        return false;
    };
    let steps = steps
        .into_iter()
        .map(|step| match step {
            WireAccessStep::Index(index) => crate::alef_context! { kind => "index", index => index },
            WireAccessStep::Field(key) => {
                crate::alef_context! { kind => "field", key => serde_json::to_string(&key).unwrap() }
            }
        })
        .collect::<Vec<_>>();
    out.push_str(&crate::e2e::template_env::render(
        "go/enum_collection_assertion.jinja",
        crate::alef_context! {
            expression => resolver.accessor(&prefix, "go", context.effective_result_var), steps => steps,
            assertion_type => &assertion.assertion_type,
            expected => super::super::json_values::json_to_go(value),
            field => serde_json::to_string(field).unwrap(),
        },
    ));
    true
}

#[cfg(test)]
#[path = "enum_collection/tests.rs"]
mod tests;

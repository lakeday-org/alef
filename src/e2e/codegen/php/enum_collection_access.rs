use super::enum_variant_access::PhpVariantAccess;
use crate::e2e::field_access::{FieldResolver, WireAccessStep};
use crate::e2e::fixture::Assertion;

pub(super) fn try_render(
    out: &mut String,
    assertion: &Assertion,
    result: &str,
    resolver: &FieldResolver,
    lowering: &PhpVariantAccess<'_>,
) -> bool {
    let Some(field) = assertion.field.as_deref() else {
        return false;
    };
    if !matches!(
        assertion.assertion_type.as_str(),
        "equals" | "count_equals" | "count_min"
    ) {
        return false;
    }
    let Some(value) = assertion.value.as_ref() else {
        return false;
    };
    if assertion.assertion_type != "equals" && value.as_u64().is_none() {
        return false;
    }
    let Some(expression) = accessor(resolver, lowering, field, result) else {
        return false;
    };
    out.push_str(&crate::e2e::template_env::render(
        "php/assertion.jinja",
        crate::alef_context! {
            assertion_type => assertion.assertion_type.as_str(), field_expr => expression,
            php_val => super::values::json_to_php(value), n => value.as_u64().unwrap_or_default(),
        },
    ));
    true
}

fn accessor(resolver: &FieldResolver, lowering: &PhpVariantAccess<'_>, field: &str, result: &str) -> Option<String> {
    let (prefix, enum_type, variant, suffix) = resolver.ir_tagged_union_split(resolver.resolve(field))?;
    if !lowering.is_json_bridged(&enum_type) || !resolver.union_variant_payload_is_collection(&enum_type, &variant) {
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
            WireAccessStep::Index(index) => format!("[{index}]"),
            WireAccessStep::Field(key) => {
                format!("->{{{}}}", super::values::json_to_php(&serde_json::Value::String(key)))
            }
        })
        .collect::<Vec<_>>();
    let expression = resolver.accessor(&prefix, "php", &format!("${result}"));
    Some(
        crate::e2e::template_env::render(
            "php/enum_collection_accessor.jinja",
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
#[path = "enum_collection_access/test_support.rs"]
pub(crate) mod test_support;

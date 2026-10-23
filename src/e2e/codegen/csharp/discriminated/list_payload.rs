use crate::e2e::field_access::FieldResolver;
use crate::e2e::fixture::Assertion;

pub(super) fn render(assertion: &Assertion, resolver: &FieldResolver, result: &str, field: &str) -> Option<String> {
    let (prefix, owner, variant, suffix) = resolver.ir_tagged_union_split(field)?;
    if !resolver.union_variant_payload_is_collection(&owner, &variant) {
        return None;
    }
    if resolver.enum_definition(&owner)?.serde_untagged {
        return render_json(assertion, resolver, result, (&prefix, &owner, &variant, &suffix));
    }
    let (_, element_type) = resolver.union_variant_payload(&owner, &variant)?;
    let container = if prefix.is_empty() {
        result.to_string()
    } else {
        resolver.accessor(&prefix, "csharp", result)
    };
    let payload = "listVariant.Value";
    let expression = if suffix.is_empty() {
        payload.to_string()
    } else {
        indexed_leaf(resolver, element_type, &suffix, payload)?
    };
    let expected = assertion.value.as_ref().map(super::super::json_to_csharp)?;
    let kind = assertion.assertion_type.as_str();
    if !matches!(kind, "count_equals" | "equals" | "field_equals") {
        return None;
    }
    Some(crate::e2e::template_env::render(
        "csharp/list_variant_assertion.jinja",
        crate::alef_context! { container => container, owner => crate::codegen::naming::csharp_type_name(&owner),
            variant => crate::codegen::naming::csharp_variant_name(&variant),
        expression => expression, expected => expected, count => kind == "count_equals" },
    ))
}

fn indexed_leaf(resolver: &FieldResolver, owner: &str, suffix: &str, payload: &str) -> Option<String> {
    let end = suffix.find(']')?;
    let element = resolver.element_accessor(&suffix[..=end], "csharp", payload);
    let tail = suffix[end + 1..].trim_start_matches('.');
    let Some(definition) = resolver.enum_definition(owner) else {
        return Some(resolver.element_accessor(tail, "csharp", &element));
    };
    let (head, remainder) = tail.split_once('.').unwrap_or((tail, ""));
    let metadata = resolver.enum_field_metadata(owner, head)?;
    let variant = definition
        .variants
        .iter()
        .find(|variant| variant.name == metadata.variant)?;
    let member = crate::backends::csharp::gen_bindings::variant_field_name(definition, variant, &metadata.definition);
    let variant_name = crate::codegen::naming::csharp_variant_name(&metadata.variant);
    let owner = crate::codegen::naming::csharp_type_name(owner);
    let narrowed = format!("(({owner}.{variant_name}){element}).{member}");
    Some(if remainder.is_empty() {
        narrowed
    } else {
        resolver.element_accessor(remainder, "csharp", &narrowed)
    })
}

fn render_json(
    assertion: &Assertion,
    resolver: &FieldResolver,
    result: &str,
    crossing: (&str, &str, &str, &str),
) -> Option<String> {
    let (prefix, owner, variant, suffix) = crossing;
    let container = if prefix.is_empty() {
        result.to_string()
    } else {
        resolver.accessor(prefix, "csharp", result)
    };
    let public_owner = crate::codegen::naming::csharp_type_name(owner);
    let payload = format!("Assert.IsType<{public_owner}>({container}).Value");
    let mut expression = payload.clone();
    for step in resolver.enum_payload_wire_steps(owner, variant, suffix)? {
        match step {
            crate::e2e::field_access::WireAccessStep::Field(key) => {
                let literal = super::super::json_to_csharp(&serde_json::Value::String(key));
                expression.push_str(&format!(".GetProperty({literal})"));
            }
            crate::e2e::field_access::WireAccessStep::Index(index) => expression.push_str(&format!("[{index}]")),
        }
    }
    let kind = assertion.assertion_type.as_str();
    let expected = assertion.value.as_ref()?;
    if !matches!(kind, "count_equals" | "equals" | "field_equals") || (kind != "count_equals" && !expected.is_string())
    {
        return None;
    }
    Some(crate::e2e::template_env::render(
        "csharp/list_variant_json_assertion.jinja",
        crate::alef_context! {
            payload => payload, expression => expression, expected => super::super::json_to_csharp(expected),
            count => kind == "count_equals",
        },
    ))
}

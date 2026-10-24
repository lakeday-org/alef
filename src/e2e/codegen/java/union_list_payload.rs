use crate::e2e::field_access::{FieldResolver, WireAccessStep};
use crate::e2e::fixture::Assertion;

use super::values::json_to_java;

struct CollectionPath {
    container: String,
    payload: String,
    expression: String,
}

pub(super) fn try_render(out: &mut String, assertion: &Assertion, result: &str, resolver: &FieldResolver) -> bool {
    let Some(field) = assertion.field.as_deref().filter(|field| !field.is_empty()) else {
        return false;
    };
    let Some(path) = collection_path(resolver, field, result) else {
        return false;
    };
    let path = match path {
        Ok(path) => path,
        Err(reason) => return emit_failure(out, &reason),
    };
    let (kind, expected) = match operation(assertion) {
        Ok(operation) => operation,
        Err(reason) => return emit_failure(out, &reason),
    };
    let nested = path.payload != path.expression;
    out.push_str(&crate::e2e::template_env::render(
        "java/list_variant_json_assertion.jinja",
        crate::alef_context! {
            error => Option::<String>::None, kind => kind, expected => expected,
            container => path.container, payload => path.payload, expression => path.expression,
            nested => nested,
        },
    ));
    true
}

fn collection_path(resolver: &FieldResolver, field: &str, result: &str) -> Option<Result<CollectionPath, String>> {
    let (prefix, owner, variant, suffix) = resolver.ir_tagged_union_split(resolver.resolve(field))?;
    if !resolver.enum_definition(&owner)?.serde_untagged
        || !resolver.union_variant_payload_is_collection(&owner, &variant)
    {
        return None;
    }
    let Some(steps) = resolver.enum_payload_wire_steps(&owner, &variant, &suffix) else {
        return Some(Err(format!("unresolved enum collection wire path: {field}")));
    };
    let container = if prefix.is_empty() {
        result.to_string()
    } else {
        resolver.accessor(&prefix, "java", result)
    };
    let payload = format!("{container}.value()");
    Some(Ok(CollectionPath {
        expression: wire_expression(&payload, steps),
        container,
        payload,
    }))
}

fn operation(assertion: &Assertion) -> Result<(&str, Option<String>), String> {
    let kind = match assertion.assertion_type.as_str() {
        "equals" | "field_equals" => "equals",
        "count_equals" => "count_equals",
        "count_min" => "count_min",
        "is_empty" => "is_empty",
        "not_empty" => "not_empty",
        kind => return Err(format!("unsupported enum collection assertion: {kind}")),
    };
    if matches!(kind, "is_empty" | "not_empty") {
        return Ok((kind, None));
    }
    let value = assertion.value.as_ref().filter(|value| {
        if kind == "equals" {
            value.is_string()
        } else {
            value.as_u64().is_some_and(|number| i32::try_from(number).is_ok())
        }
    });
    value
        .map(|value| (kind, Some(json_to_java(value))))
        .ok_or_else(|| format!("invalid value for enum collection assertion: {kind}"))
}

fn emit_failure(out: &mut String, reason: &str) -> bool {
    out.push_str(&crate::e2e::template_env::render(
        "java/list_variant_json_assertion.jinja",
        crate::alef_context! { error => json_to_java(&serde_json::Value::String(reason.to_string())) },
    ));
    true
}

fn wire_expression(payload: &str, steps: Vec<WireAccessStep>) -> String {
    let mut expression = payload.to_string();
    for step in steps {
        match step {
            WireAccessStep::Field(key) => {
                let literal = json_to_java(&serde_json::Value::String(key));
                expression.push_str(&format!(".get({literal})"));
            }
            WireAccessStep::Index(index) => expression.push_str(&format!(".get({index})")),
        }
    }
    expression
}

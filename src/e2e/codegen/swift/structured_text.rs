mod predicate;

use crate::e2e::field_access::{FieldResolver, WireAccessStep};
use crate::e2e::fixture::Assertion;

pub(super) fn render(out: &mut String, assertion: &Assertion, resolver: &FieldResolver, result: &str) -> bool {
    if render_supported(out, assertion, resolver, result) {
        return true;
    }
    let Some(field) = assertion.field.as_deref() else {
        return false;
    };
    let Some((prefix, owner, _, _)) = resolver.ir_tagged_union_split(resolver.resolve(field)) else {
        return false;
    };
    if prefix.is_empty() || (!resolver.is_text_projected_enum(&owner) && !resolver.is_display_as_text(&prefix)) {
        return false;
    }
    out.push_str(&crate::e2e::template_env::render(
        "swift/structured_text_unavailable.jinja",
        crate::alef_context! { message => crate::e2e::codegen::field_skip::FieldSkip::DisplayTextUnionVariantNotReadableInSwift.message(field) },
    ));
    true
}

fn render_supported(out: &mut String, assertion: &Assertion, resolver: &FieldResolver, result: &str) -> bool {
    let Some(predicate) = predicate::build(assertion) else {
        return false;
    };
    let Some(field) = assertion.field.as_deref() else {
        return false;
    };
    let Some((prefix, owner, variant, suffix)) = resolver.ir_tagged_union_split(resolver.resolve(field)) else {
        return false;
    };
    if !resolver.is_text_projected_enum(&owner) {
        return false;
    }
    let Some(steps) = resolver.enum_payload_wire_steps(&owner, &variant, &suffix) else {
        return false;
    };
    let Some(opaque) = resolver.swift_lossless_text_leaf_is_opaque(&prefix) else {
        return false;
    };
    let leaf_expression = resolver.accessor(&prefix, "swift", result);
    let Some((setup, leaf_expression, _)) =
        super::accessors::materialise_vec_temporaries(&leaf_expression, "structured_text")
    else {
        return false;
    };
    let source = json_source(&leaf_expression, &prefix, opaque, result);
    let expression = wire_expression(resolver, &owner, &variant, steps);
    out.push_str(&crate::e2e::template_env::render(
        "swift/structured_text_assertion.jinja",
        crate::alef_context! { setup => setup, source => source, expression => expression,
        predicate => predicate },
    ));
    true
}

fn field_read(value: &str, key: &str) -> String {
    let key = super::values::json_to_swift(&serde_json::Value::String(key.into()));
    format!("(try XCTUnwrap({value} as? [String: Any]))[{key}]")
}

fn json_source(leaf_expression: &str, prefix: &str, opaque: bool, result: &str) -> String {
    if opaque {
        let (parent, _) = leaf_expression.rsplit_once('.').unwrap_or((result, leaf_expression));
        let leaf_name = prefix.rsplit('.').next().unwrap_or(prefix);
        let getter = crate::backends::swift::gen_rust_crate::text_projection::json_getter_name(leaf_name);
        let optional = parent.contains("?.") || parent.contains("?[") || parent.ends_with('?');
        if optional {
            format!(r#"Data(({parent}.{getter}().toString() ?? "null").utf8)"#)
        } else {
            format!("Data({parent}.{getter}().toString().utf8)")
        }
    } else {
        format!("try JSONEncoder().encode({leaf_expression})")
    }
}

fn wire_expression(resolver: &FieldResolver, owner: &str, variant: &str, steps: Vec<WireAccessStep>) -> String {
    let mut expression = "payload".to_string();
    for key in resolver.enum_payload_wire_prefix(owner, variant).unwrap_or_default() {
        expression = field_read(&expression, &key);
    }
    for step in steps {
        expression = match step {
            WireAccessStep::Field(key) => field_read(&expression, &key),
            WireAccessStep::Index(index) => {
                format!("try XCTUnwrap((try XCTUnwrap({expression} as? [Any])).dropFirst({index}).first)")
            }
        };
    }
    expression
}

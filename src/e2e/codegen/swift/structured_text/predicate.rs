use crate::e2e::fixture::Assertion;

#[derive(serde::Serialize)]
pub(super) struct Predicate {
    kind: String,
    expected: String,
    value_type: &'static str,
    values: Vec<String>,
}

pub(super) fn build(assertion: &Assertion) -> Option<Predicate> {
    let mut predicate = Predicate {
        kind: assertion.assertion_type.clone(),
        expected: String::new(),
        value_type: "",
        values: Vec::new(),
    };
    match assertion.assertion_type.as_str() {
        "not_empty" | "is_empty" => {}
        "contains_all" => {
            predicate.values = assertion
                .values
                .as_ref()?
                .iter()
                .map(|value| {
                    value.as_str()?;
                    Some(super::super::values::json_to_swift(value))
                })
                .collect::<Option<_>>()?;
        }
        "contains" => {
            let value = assertion.value.as_ref()?;
            value.as_str()?;
            predicate.expected = super::super::values::json_to_swift(value);
        }
        "count_min" | "count_equals" | "min_length" => {
            predicate.expected = assertion.value.as_ref()?.as_u64()?.to_string();
        }
        "equals" | "field_equals" | "greater_than" | "greater_than_or_equal" => {
            let value = assertion.value.as_ref()?;
            predicate.value_type = value_type(value)?;
            if !matches!(assertion.assertion_type.as_str(), "equals" | "field_equals") && !value.is_number() {
                return None;
            }
            predicate.expected = super::super::values::json_to_swift(value);
            if value.is_number() {
                set_number(&mut predicate, value);
            }
        }
        _ => return None,
    }
    Some(predicate)
}

fn set_number(predicate: &mut Predicate, value: &serde_json::Value) {
    predicate.expected = super::super::values::json_to_swift(&serde_json::Value::String(value.to_string()));
}

fn value_type(value: &serde_json::Value) -> Option<&'static str> {
    match value {
        serde_json::Value::String(_) => Some("String"),
        serde_json::Value::Bool(_) => Some("Bool"),
        serde_json::Value::Number(_) => Some("NSNumber"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn numeric_predicates_preserve_signed_unsigned_and_float_domains() {
        for value in [
            serde_json::json!(i64::MIN),
            serde_json::json!(u64::MAX),
            serde_json::json!(1.25),
        ] {
            let expected = serde_json::to_string(&value.to_string()).unwrap();
            let predicate = super::build(&crate::e2e::fixture::Assertion {
                assertion_type: "equals".into(),
                value: Some(value),
                ..Default::default()
            })
            .unwrap();
            assert_eq!(predicate.value_type, "NSNumber");
            assert_eq!(predicate.expected, expected);
        }
    }
}

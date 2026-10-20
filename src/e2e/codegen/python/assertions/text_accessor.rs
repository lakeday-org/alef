//! Text assertions on a `fields_display_as_text` field.
//!
//! Such a field holds a data enum (for example a message `content` with a text variant and a
//! parts variant). The pyo3 binding exposes the text variant as the `.text` property. A
//! comparison through `str()` reads the enum's string form and passes even when `.text`
//! returns `None`, so the comparison reads `.text` instead. ~keep

use crate::e2e::fixture::Assertion;

use super::super::json::value_to_python_string;

/// Render a text assertion through the `.text` accessor of `field_access`.
///
/// Returns `false` and writes nothing when the assertion is not a text comparison against string
/// values. The caller then renders it as for any other field: `not_empty`, `is_empty` and the
/// length checks ask whether a value is present, and a parts value is present but has no text.
pub(super) fn try_render(out: &mut String, assertion: &Assertion, field_access: &str, field_is_optional: bool) -> bool {
    let kind = assertion.assertion_type.as_str();
    let expected: Vec<&serde_json::Value> = match kind {
        "equals" | "contains" | "starts_with" | "ends_with" => assertion.value.iter().collect(),
        "contains_all" | "contains_any" => assertion.values.iter().flatten().collect(),
        "not_contains" => assertion.expected_values(),
        _ => return false,
    };
    if expected.is_empty() || !expected.iter().all(|v| v.is_string()) {
        return false;
    }
    let literals: Vec<String> = expected.iter().map(|v| value_to_python_string(v)).collect();
    let text = format!("_alef_e2e_text({field_access}.text)");
    // `.text` on `None` raises `AttributeError`, so an optional field is checked for presence
    // first. `not_contains` keeps its meaning for an absent value: nothing is contained. ~keep
    let guard = if field_is_optional {
        if kind == "not_contains" {
            format!("{field_access} is None or ")
        } else {
            out.push_str(&format!("    assert {field_access} is not None\n"));
            String::new()
        }
    } else {
        String::new()
    };
    match kind {
        "equals" => out.push_str(&format!("    assert {text}.lower() == {}.lower()\n", literals[0])),
        "contains" | "contains_all" => {
            for literal in &literals {
                out.push_str(&format!("    assert {literal}.lower() in {text}.lower()\n"));
            }
        }
        "contains_any" => {
            let list = literals.join(", ");
            out.push_str(&format!(
                "    assert any(v.lower() in {text}.lower() for v in [{list}])\n"
            ));
        }
        "not_contains" => {
            for literal in &literals {
                out.push_str(&format!("    assert {guard}{literal}.lower() not in {text}.lower()\n"));
            }
        }
        "starts_with" => out.push_str(&format!("    assert {text}.startswith({})\n", literals[0])),
        "ends_with" => out.push_str(&format!("    assert {text}.endswith({})\n", literals[0])),
        _ => return false,
    }
    true
}

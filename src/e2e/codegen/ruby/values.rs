//! Ruby e2e value/literal helpers.

use heck::ToUpperCamelCase;

/// Convert a module path (e.g., "demo_markup") to Ruby PascalCase module name
/// (e.g., "DemoMarkup").
pub(super) fn ruby_module_name(module_path: &str) -> String {
    module_path.to_upper_camel_case()
}

/// Qualify a config-supplied Ruby class name (`options_type`, an adapter's `request_type`, ...)
/// under the call's module, unless the name already names a module.
///
/// `options_type` is contractually a bare class name -- `csharp` and `go` use the configured
/// name verbatim with no module/package concatenation at all, and every language that *does*
/// concatenate a qualifier (PHP prepends its namespace the same way) must not re-qualify a name
/// that already carries one. Without this guard, a value naming its own gem's module
/// (`"Sample::DocumentRequest"`) or a deliberately different one (`"Zzz::DocumentRequest"`) gets
/// prefixed again into `"Sample::Sample::DocumentRequest"`, which does not resolve. ~keep
pub(super) fn qualify_ruby_type(module_name: &str, type_name: &str) -> String {
    if type_name.contains("::") {
        type_name.to_string()
    } else {
        format!("{}::{type_name}", ruby_module_name(module_name))
    }
}

/// Convert a `serde_json::Value` to a Ruby literal string, preferring single quotes.
pub(super) fn json_to_ruby(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => crate::e2e::escape::ruby_string_literal(s),
        serde_json::Value::Bool(true) => "true".to_string(),
        serde_json::Value::Bool(false) => "false".to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => "nil".to_string(),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(json_to_ruby).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(map) => {
            let items: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{} => {}", crate::e2e::escape::ruby_string_literal(k), json_to_ruby(v)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
    }
}

pub(super) fn typed_field_value(
    value: &serde_json::Value,
    field_name: &str,
    containing_type: &str,
    type_defs: &[crate::core::ir::TypeDef],
    module_name: &str,
) -> String {
    let type_name = containing_type.rsplit("::").next().unwrap_or(containing_type);
    let field = type_defs.iter().find(|ty| ty.name == type_name).and_then(|ty| {
        ty.fields.iter().find(|field| {
            field.name == field_name
                || crate::codegen::naming::wire_field_name(
                    &field.name,
                    field.serde_rename.as_deref(),
                    ty.serde_rename_all.as_deref(),
                ) == field_name
        })
    });
    field.map_or_else(
        || json_to_ruby(value),
        |field| {
            if field.optional && value.is_null() {
                "nil".into()
            } else {
                typed_ruby_value(value, &field.ty, type_defs, module_name)
            }
        },
    )
}

fn typed_ruby_value(
    value: &serde_json::Value,
    ty: &crate::core::ir::TypeRef,
    type_defs: &[crate::core::ir::TypeDef],
    module_name: &str,
) -> String {
    use crate::core::ir::TypeRef;
    match (value, ty) {
        (value, TypeRef::Json) => crate::e2e::escape::ruby_string_literal(&value.to_string()),
        (serde_json::Value::Null, TypeRef::Optional(_)) => "nil".into(),
        (value, TypeRef::Optional(inner)) => typed_ruby_value(value, inner, type_defs, module_name),
        (serde_json::Value::Array(values), TypeRef::Vec(inner)) => {
            let items: Vec<_> = values
                .iter()
                .map(|value| typed_ruby_value(value, inner, type_defs, module_name))
                .collect();
            format!("[{}]", items.join(", "))
        }
        (serde_json::Value::Object(values), TypeRef::Map(_, inner)) => {
            let entries: Vec<_> = values
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{} => {}",
                        crate::e2e::escape::ruby_string_literal(key),
                        typed_ruby_value(value, inner, type_defs, module_name)
                    )
                })
                .collect();
            format!("{{ {} }}", entries.join(", "))
        }
        (serde_json::Value::Object(_), TypeRef::Named(name)) => {
            render_typed_object(value, name, type_defs, module_name).unwrap_or_else(|| json_to_ruby(value))
        }
        _ => json_to_ruby(value),
    }
}

fn render_typed_object(
    value: &serde_json::Value,
    type_name: &str,
    type_defs: &[crate::core::ir::TypeDef],
    module_name: &str,
) -> Option<String> {
    let type_def = type_defs.iter().find(|ty| ty.name == type_name)?;
    let fields: Vec<_> = value
        .as_object()?
        .iter()
        .map(|(key, value)| typed_object_field(key, value, type_def, type_defs, module_name))
        .collect();
    Some(
        crate::e2e::template_env::render(
            "ruby/typed_constructor.jinja",
            crate::alef_context! { type_name => qualify_ruby_type(module_name, type_name), fields => fields },
        )
        .trim_end()
        .to_string(),
    )
}

fn typed_object_field(
    key: &str,
    value: &serde_json::Value,
    type_def: &crate::core::ir::TypeDef,
    type_defs: &[crate::core::ir::TypeDef],
    module_name: &str,
) -> minijinja::Value {
    let field = type_def.fields.iter().find(|field| {
        field.name == key
            || crate::codegen::naming::wire_field_name(
                &field.name,
                field.serde_rename.as_deref(),
                type_def.serde_rename_all.as_deref(),
            ) == key
    });
    let name = crate::codegen::naming::public_field_name(
        crate::core::config::Language::Ruby,
        field.map_or(key, |field| field.name.as_str()),
        None,
    );
    let value = typed_field_value(value, key, &type_def.name, type_defs, module_name);
    crate::alef_context! { name => name, value => value }
}

/// Classify a fixture string value that maps to a `bytes` argument.
///
/// Returns true if the value looks like a file path (e.g. "pdf/fake_memo.pdf").
/// File paths have the pattern: alphanumeric/something.extension
pub(super) fn is_file_path(s: &str) -> bool {
    if s.starts_with('<') || s.starts_with('{') || s.starts_with('[') || s.contains(' ') {
        return false;
    }

    let first = s.chars().next().unwrap_or('\0');
    if (first.is_ascii_alphanumeric() || first == '_')
        && let Some(slash_pos) = s.find('/')
        && slash_pos > 0
    {
        let after_slash = &s[slash_pos + 1..];
        if after_slash.contains('.') && !after_slash.is_empty() {
            return true;
        }
    }

    false
}

/// Check if a string looks like base64-encoded data.
///
/// If it's not a file path or inline text, assume it's base64.
pub(super) fn is_base64(s: &str) -> bool {
    if s.starts_with('<') || s.starts_with('{') || s.starts_with('[') || s.contains(' ') {
        return false;
    }

    if is_file_path(s) {
        return false;
    }

    true
}

#[cfg(test)]
mod qualify_ruby_type_tests {
    use super::qualify_ruby_type;

    #[test]
    fn bare_name_is_qualified_under_the_module() {
        assert_eq!(
            qualify_ruby_type("sample", "DocumentRequest"),
            "Sample::DocumentRequest"
        );
    }

    #[test]
    fn name_already_qualified_under_the_same_module_is_not_doubled() {
        assert_eq!(
            qualify_ruby_type("sample", "Sample::DocumentRequest"),
            "Sample::DocumentRequest"
        );
    }

    #[test]
    fn name_qualified_under_a_foreign_module_is_preserved_verbatim() {
        assert_eq!(
            qualify_ruby_type("sample", "Zzz::DocumentRequest"),
            "Zzz::DocumentRequest"
        );
    }
}

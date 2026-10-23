use super::{ErrorDef, ErrorVariant, SwiftMapper, associated_values};
use crate::core::ir::{PrimitiveType, TypeRef};
use std::collections::BTreeSet;

#[derive(serde::Serialize)]
struct Case {
    name: String,
    pattern: String,
    expression: String,
}

fn literal(value: &str) -> String {
    let mut result = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            control if control.is_control() => result.push_str(&format!("\\u{{{:x}}}", u32::from(control))),
            other => result.push(other),
        }
    }
    result.push('"');
    result
}

fn display_field(variant: &ErrorVariant, name: &str, mapper: &SwiftMapper) -> Option<String> {
    let values = associated_values(variant, mapper);
    let value = if let Ok(index) = name.parse::<usize>() {
        values
            .iter()
            .find(|value| value.field.is_some_and(|field| field.name == format!("_{index}")))
    } else {
        values
            .iter()
            .find(|value| value.field.is_some_and(|field| field.name == name))
    }?;
    let field = value.field?;
    if field.binding_excluded || field.optional {
        return None;
    }
    matches!(
        field.ty,
        TypeRef::String
            | TypeRef::Primitive(
                PrimitiveType::Bool
                    | PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::Isize
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::Usize
            )
    )
    .then(|| value.label.clone())
}

fn placeholder(characters: &mut impl Iterator<Item = char>) -> Option<String> {
    let mut name = String::new();
    for character in characters.by_ref() {
        match character {
            '}' => return Some(name),
            '{' => return None,
            other => name.push(other),
        }
    }
    None
}

fn template_expression(variant: &ErrorVariant, mapper: &SwiftMapper) -> Option<(String, BTreeSet<String>)> {
    let template = variant.message_template.as_deref()?;
    let mut characters = template.chars().peekable();
    let mut text = String::new();
    let mut pieces = Vec::new();
    let mut bindings = BTreeSet::new();
    while let Some(character) = characters.next() {
        if matches!(character, '{' | '}') && characters.peek() == Some(&character) {
            characters.next();
            text.push(character);
            continue;
        }
        match character {
            '{' => {
                pieces.push(literal(&text));
                text.clear();
                let name = placeholder(&mut characters)?;
                let binding = display_field(variant, &name, mapper)?;
                pieces.push(format!("String(describing: {binding})"));
                bindings.insert(binding);
            }
            '}' => return None,
            other => text.push(other),
        }
    }
    pieces.push(literal(&text));
    Some((pieces.join(" + "), bindings))
}

fn description_case(variant: &ErrorVariant, mapper: &SwiftMapper) -> Case {
    let values = if variant.is_unit || variant.fields.is_empty() {
        Vec::new()
    } else {
        associated_values(variant, mapper)
    };
    let synthetic_message = values.iter().find(|value| value.field.is_none());
    let (expression, bindings) = if let Some(value) = synthetic_message {
        (value.label.clone(), BTreeSet::from([value.label.clone()]))
    } else if let Some(rendered) = template_expression(variant, mapper) {
        rendered
    } else if let Some(value) = values.iter().find(|value| value.label == "message") {
        (
            format!("String(describing: {})", value.label),
            BTreeSet::from([value.label.clone()]),
        )
    } else {
        (literal(&variant.name), BTreeSet::new())
    };
    let pattern = if values.is_empty() {
        String::new()
    } else {
        format!(
            "({})",
            values
                .iter()
                .map(|value| {
                    if bindings.contains(&value.label) {
                        format!("{}: let {}", value.label, value.label)
                    } else {
                        format!("{}: _", value.label)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Case {
        name: super::super::enums::swift_variant_case(&variant.name),
        pattern,
        expression,
    }
}

pub(super) fn emit(error: &ErrorDef, name: &str, mapper: &SwiftMapper, out: &mut String) {
    let mut cases = error
        .variants
        .iter()
        .map(|variant| description_case(variant, mapper))
        .collect::<Vec<_>>();
    if !cases.iter().any(|case| case.name == "validation") {
        cases.push(Case {
            name: "validation".into(),
            pattern: "(message: let message, source: _)".into(),
            expression: "message".into(),
        });
    }
    out.push_str(&crate::backends::swift::template_env::render(
        "swift_error_description.swift.jinja",
        crate::alef_context! { name => name, cases => cases },
    ));
}

#[cfg(test)]
#[path = "errors_description_tests.rs"]
mod tests;

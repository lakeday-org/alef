use crate::core::ir::{PrimitiveType, TypeDef, TypeRef};
use crate::e2e::fixture::{Assertion, Fixture};

#[derive(Default)]
pub(super) struct UsageProbe {
    type_snake: String,
    fields: Vec<Counter>,
}

#[derive(serde::Serialize)]
struct Counter {
    name: String,
    paths: Vec<String>,
    local: String,
    c_type: &'static str,
    signed: bool,
}

impl UsageProbe {
    pub(super) fn has_fields(&self) -> bool {
        !self.fields.is_empty()
    }

    pub(super) fn resolve(fixture: &Fixture, types: &[TypeDef], item: &str) -> Self {
        let find = |name: &str| types.iter().find(|ty| ty.name == name && !ty.binding_excluded);
        let Some(owner) = find(item)
            .and_then(|ty| crate::codegen::shared::binding_fields(&ty.fields).find(|field| field.name == "usage"))
            .and_then(|field| named(&field.ty))
            .and_then(find)
        else {
            return Self::default();
        };
        let mut fields: Vec<Counter> = Vec::new();
        for name in fixture
            .assertions
            .iter()
            .filter_map(|assertion| assertion.field.as_deref())
            .filter_map(|field| field.strip_prefix("usage."))
        {
            let Some(counter) = counter(owner, name) else {
                continue;
            };
            if let Some(existing) = fields.iter_mut().find(|existing| existing.name == counter.name) {
                existing.paths.push(name.into());
            } else {
                fields.push(counter);
            }
        }
        Self {
            type_snake: crate::codegen::naming::pascal_to_snake(&owner.name),
            fields,
        }
    }

    pub(super) fn render_declarations(&self, out: &mut String) {
        if self.has_fields() {
            out.push_str(&crate::e2e::template_env::render(
                "c/usage_declarations.jinja",
                crate::alef_context! { fields => &self.fields },
            ));
        }
    }

    pub(super) fn render_capture(&self, out: &mut String, prefix: &str, item: &str, result: &str, prefix_upper: &str) {
        if self.has_fields() {
            out.push_str(&crate::e2e::template_env::render(
                "c/usage_capture.jinja",
                crate::alef_context! {
                    fields => self.fields.iter().map(|field| crate::alef_context! {
                    local => &field.local, getter => crate::codegen::naming::ffi_field_accessor_symbol(prefix,&self.type_snake,&field.name),
                }).collect::<Vec<_>>(), usage_type => &self.type_snake, prefix => prefix,
                    item => item, result => result, prefix_upper => prefix_upper,
                },
            ));
        }
    }

    pub(super) fn render_assertion(&self, out: &mut String, assertion: &Assertion) -> bool {
        let Some(counter) = assertion
            .field
            .as_deref()
            .and_then(|field| field.strip_prefix("usage."))
            .and_then(|name| {
                self.fields
                    .iter()
                    .find(|field| field.paths.iter().any(|path| path == name))
            })
        else {
            return false;
        };
        let operator = match assertion.assertion_type.as_str() {
            "equals" => "==",
            "greater_than" => ">",
            "greater_than_or_equal" => ">=",
            "less_than" => "<",
            "less_than_or_equal" => "<=",
            _ => return false,
        };
        let Some(number) = assertion.value.as_ref().and_then(|value| value.as_number()) else {
            return false;
        };
        let Some((literal, constant)) = numeric_expectation(number, counter.signed, operator) else {
            return false;
        };
        out.push_str(&crate::e2e::template_env::render(
            "c/usage_assertion.jinja",
            crate::alef_context! {
                local => &counter.local, operator => operator, literal => literal, constant => constant,
            },
        ));
        true
    }
}

fn counter(owner: &TypeDef, path: &str) -> Option<Counter> {
    let field = crate::codegen::shared::binding_fields(&owner.fields).find(|field| {
        field.name == path
            || crate::codegen::naming::wire_field_name(
                &field.name,
                field.serde_rename.as_deref(),
                owner.serde_rename_all.as_deref(),
            ) == path
    })?;
    let TypeRef::Primitive(primitive) = &field.ty else {
        return None;
    };
    let (c_type, signed) = integer_type(primitive)?;
    Some(Counter {
        name: field.name.clone(),
        paths: vec![path.into()],
        local: format!("last_usage_{}", field.name),
        c_type,
        signed,
    })
}

fn named(ty: &TypeRef) -> Option<&str> {
    match ty {
        TypeRef::Named(name) => name.rsplit("::").next(),
        TypeRef::Optional(inner) => named(inner),
        _ => None,
    }
}

fn integer_type(primitive: &PrimitiveType) -> Option<(&'static str, bool)> {
    use PrimitiveType::*;
    Some(match primitive {
        U8 => ("uint8_t", false),
        U16 => ("uint16_t", false),
        U32 => ("uint32_t", false),
        U64 => ("uint64_t", false),
        Usize => ("uintptr_t", false),
        I8 => ("int8_t", true),
        I16 => ("int16_t", true),
        I32 => ("int32_t", true),
        I64 => ("int64_t", true),
        Isize => ("intptr_t", true),
        _ => return None,
    })
}

fn numeric_expectation(number: &serde_json::Number, signed: bool, operator: &str) -> Option<(String, Option<bool>)> {
    if let Some(value) = number.as_i64() {
        if value < 0 && !signed {
            return Some((String::new(), Some(matches!(operator, ">" | ">="))));
        }
        let literal = if value == i64::MIN {
            "(-9223372036854775807LL - 1)".into()
        } else {
            format!("{value}{}", if signed { "LL" } else { "ULL" })
        };
        return Some((literal, None));
    }
    let value = number.as_u64()?;
    Some(if signed {
        (String::new(), Some(matches!(operator, "<" | "<=")))
    } else {
        (format!("{value}ULL"), None)
    })
}

#[cfg(test)]
mod tests;

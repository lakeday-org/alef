use super::super::conversions::primitive_name;
use crate::core::ir::{CoreWrapper, FieldDef, PrimitiveType, TypeRef};

struct FieldExpression {
    value: String,
    fallible: bool,
}

impl FieldExpression {
    fn plain(value: String) -> Self {
        Self { value, fallible: false }
    }

    fn finish(self) -> String {
        if self.fallible {
            format!("{}?", self.value)
        } else {
            self.value
        }
    }
}

pub(super) fn field_from_expr(field: &FieldDef, variable: &str) -> String {
    prepare(field, variable).finish()
}

fn wrapped(field: &FieldDef, ty: TypeRef, variable: &str) -> FieldExpression {
    prepare(
        &FieldDef {
            ty,
            optional: false,
            ..field.clone()
        },
        variable,
    )
}

fn optional(field: &FieldDef, inner: TypeRef, variable: &str) -> FieldExpression {
    let expression = wrapped(field, inner, "v");
    let value = if expression.value == "v.clone()" {
        format!("{variable}.clone()")
    } else if expression.fallible {
        format!("{variable}.as_ref().map(|v| {}).transpose()", expression.value)
    } else {
        format!("{variable}.as_ref().map(|v| {})", expression.value)
    };
    FieldExpression {
        value,
        fallible: expression.fallible,
    }
}

fn sequence(field: &FieldDef, inner: &TypeRef, variable: &str) -> FieldExpression {
    let expression = wrapped(field, inner.clone(), "x");
    let collect = if expression.fallible {
        "collect::<Result<_, String>>()"
    } else {
        "collect()"
    };
    FieldExpression {
        value: format!("{variable}.iter().map(|x| {}).{collect}", expression.value),
        fallible: expression.fallible,
    }
}

fn prepare(field: &FieldDef, variable: &str) -> FieldExpression {
    if field.optional && !matches!(field.ty, TypeRef::Optional(_)) {
        return optional(field, field.ty.clone(), variable);
    }
    match &field.ty {
        TypeRef::Optional(inner) => optional(field, *inner.clone(), variable),
        TypeRef::Vec(inner) => sequence(field, inner, variable),
        TypeRef::String if field.core_wrapper == CoreWrapper::StaticStr => FieldExpression {
            value: format!("lookup_static_str({variable})"),
            fallible: true,
        },
        _ => FieldExpression::plain(scalar(field, variable)),
    }
}

fn scalar(field: &FieldDef, variable: &str) -> String {
    match &field.ty {
        TypeRef::Primitive(PrimitiveType::I64 | PrimitiveType::F64 | PrimitiveType::Bool) => format!("*{variable}"),
        TypeRef::Primitive(primitive) => format!("*{variable} as {}", primitive_name(primitive)),
        TypeRef::Duration => format!("std::time::Duration::from_millis(*{variable} as u64)"),
        TypeRef::String if field.core_wrapper == CoreWrapper::Cow => {
            format!("std::borrow::Cow::Owned({variable}.clone())")
        }
        TypeRef::Char => format!("{variable}.chars().next().unwrap_or('\\0')"),
        _ => format!("{variable}.clone()"),
    }
}

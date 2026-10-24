use super::dto::swift_field_type;
use super::zero_arg_default::swift_constructor_default_literal;
use crate::backends::swift::naming::swift_source_ident;
use crate::backends::swift::type_map::SwiftMapper;
use crate::core::ir::{FieldDef, TypeDef, TypeRef};
use anyhow::Context;
use heck::ToLowerCamelCase;
use serde::Serialize;

pub(super) fn parameters(owner: &TypeDef, mapper: &SwiftMapper, fields: &[&FieldDef]) -> Vec<String> {
    fields
        .iter()
        .map(|field| {
            let name = swift_source_ident(&field.name.to_lower_camel_case());
            let already_optional = matches!(&field.ty, TypeRef::Optional(_));
            let swift_ty = swift_field_type(mapper, &field.ty);
            let optional_suffix = if field.optional && !already_optional { "?" } else { "" };
            match field_default(owner, field) {
                Some(value) => format!("{name}: {swift_ty}{optional_suffix} = {value}"),
                None => format!("{name}: {swift_ty}{optional_suffix}"),
            }
        })
        .collect()
}

fn field_default(owner: &TypeDef, field: &FieldDef) -> Option<String> {
    swift_constructor_default_literal(owner, field)
        .or_else(|| (field.optional || matches!(field.ty, TypeRef::Optional(_))).then(|| "nil".into()))
}

#[derive(Debug, Serialize)]
pub(super) struct LegacyInitializer {
    params: String,
    args: String,
}

pub(super) fn recorded_initializers(
    owner: &TypeDef,
    fields: &[&FieldDef],
    parameters: &[String],
) -> anyhow::Result<Vec<LegacyInitializer>> {
    std::iter::once(owner.legacy_constructor_fields())
        .filter(|signature| !signature.is_empty())
        .chain(owner.constructor_history().iter().map(Vec::as_slice))
        .map(|signature| recorded_initializer(owner, signature, fields, parameters))
        .filter_map(Result::transpose)
        .collect()
}

fn recorded_initializer(
    owner: &TypeDef,
    legacy: &[String],
    fields: &[&FieldDef],
    parameters: &[String],
) -> anyhow::Result<Option<LegacyInitializer>> {
    if fields.iter().map(|field| &field.name).eq(legacy.iter()) {
        return Ok(None);
    }
    let params = legacy
        .iter()
        .map(|name| {
            let index = fields.iter().position(|field| &field.name == name).with_context(|| {
                format!(
                    "Swift DTO {} recorded initializer field `{name}` is unavailable",
                    owner.name
                )
            })?;
            parameters.get(index).cloned().with_context(|| {
                format!(
                    "Swift DTO {} recorded initializer parameter `{name}` is unavailable",
                    owner.name
                )
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let args = fields
        .iter()
        .map(|field| {
            let name = swift_source_ident(&field.name.to_lower_camel_case());
            let value = if legacy.contains(&field.name) {
                name.clone()
            } else {
                field_default(owner, field).with_context(|| {
                    format!(
                        "Swift DTO {} initializer extension `{}` has no representable Swift default",
                        owner.name, field.name
                    )
                })?
            };
            Ok(format!("{name}: {value}"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(Some(LegacyInitializer {
        params: params.join(", "),
        args: args.join(", "),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_recorded_field_returns_a_contextual_error() {
        let mut owner = TypeDef {
            name: "Options".into(),
            ..Default::default()
        };
        owner.version.legacy_constructor_fields = vec!["missing".into()];
        let error = recorded_initializers(&owner, &[], &[]).unwrap_err().to_string();
        assert_eq!(
            error,
            "Swift DTO Options recorded initializer field `missing` is unavailable"
        );
    }
}

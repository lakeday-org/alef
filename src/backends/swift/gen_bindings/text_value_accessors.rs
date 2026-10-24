//! The public accessor that returns the whole value of a text-projected field.
//!
//! The getter of a field listed in `untagged_union_text_types` returns the plain text of the
//! union, so the parts of a list variant are lost. The bridge crate emits a second getter that
//! returns the value as JSON. This module emits an extension of the bridge type that decodes that
//! JSON into the Swift union type.

use crate::backends::swift::gen_rust_crate::type_bridge::text_projected_getter_type;
use crate::backends::swift::gen_rust_crate::wrappers::is_unbridgeable_getter;
use crate::backends::swift::naming::swift_source_ident;
use crate::codegen::generators::type_paths::build_type_path_lookup;
use crate::core::ir::{ApiSurface, FieldDef, TypeRef};
use std::collections::{HashMap, HashSet};

pub(super) struct AccessorConfig<'a> {
    pub text_types: &'a [String],
    pub rename_fields: Option<&'a HashMap<String, String>>,
}

pub(super) fn config(config: &crate::core::config::ResolvedCrateConfig) -> AccessorConfig<'_> {
    AccessorConfig {
        text_types: &config.untagged_union_text_types,
        rename_fields: config.swift.as_ref().map(|swift| &swift.rename_fields),
    }
}

pub(super) fn emit(
    api: &ApiSurface,
    config: AccessorConfig<'_>,
    exclude_types: &HashSet<String>,
    exclude_fields: &HashSet<String>,
    configured_features: &HashSet<&str>,
    out: &mut String,
) -> anyhow::Result<()> {
    if config.text_types.is_empty() {
        return Ok(());
    }
    let configured = super::super::gen_rust_crate::text_projection::serde_text_types(api, config.text_types);
    let type_paths = build_type_path_lookup(api);
    let no_serde_names: HashSet<&str> = api
        .types
        .iter()
        .filter(|t| !t.has_serde)
        .map(|t| t.name.as_str())
        .collect();
    for ty in api
        .types
        .iter()
        .filter(|t| !t.is_trait && t.has_serde && !exclude_types.contains(&t.name))
    {
        let bridged: Vec<_> = ty
            .fields
            .iter()
            .filter(|field| {
                !is_unbridgeable_getter(
                    ty,
                    field,
                    exclude_fields,
                    &type_paths,
                    &no_serde_names,
                    configured_features,
                )
            })
            .collect();
        let accessors = type_accessors(ty, &bridged, &configured, config.rename_fields)?;
        if accessors.is_empty() {
            continue;
        }
        out.push_str(&crate::backends::swift::template_env::render(
            "swift_text_value_accessors.swift.jinja",
            crate::alef_context! { type_name => &ty.name, accessors => accessors },
        ));
        out.push('\n');
    }
    Ok(())
}

fn type_accessors(
    ty: &crate::core::ir::TypeDef,
    bridged: &[&FieldDef],
    configured: &HashSet<&str>,
    rename_fields: Option<&HashMap<String, String>>,
) -> anyhow::Result<Vec<minijinja::Value>> {
    for field in bridged {
        if !field.serde_skip && text_projected_getter_type(field, configured).is_some() {
            reject_name_collision(ty, field, bridged, rename_fields)?;
        }
    }
    let accessors: Vec<_> = bridged
            .iter()
            .filter_map(|field| {
                if field.serde_skip { return None; }
                text_projected_getter_type(field, configured)?;
                let named = match &field.ty {
                    TypeRef::Optional(inner) => inner.as_ref(),
                    other => other,
                };
                let TypeRef::Named(enum_name) = named else { return None };
                let optional = field.optional || matches!(field.ty, TypeRef::Optional(_));
                let json_getter = crate::backends::swift::gen_rust_crate::text_projection::json_getter_name(&field.name);
                let renamed = rename_fields.and_then(|renames| renames.get(&format!("{}.{}", ty.name, field.name))).map(String::as_str);
                let enum_name = swift_source_ident(enum_name);
                Some(crate::alef_context! {
                    field => crate::codegen::naming::public_field_name(crate::core::config::Language::Swift, &field.name, renamed),
                    name => crate::backends::swift::naming::text_value_accessor_name(&field.name, renamed),
                    json_getter => json_getter,
                    value_type => if optional { format!("{enum_name}?") } else { enum_name.clone() },
                    enum_name => enum_name,
                    optional => optional,
                })
            })
            .collect();
    Ok(accessors)
}

fn reject_name_collision(
    ty: &crate::core::ir::TypeDef,
    field: &FieldDef,
    bridged: &[&FieldDef],
    renames: Option<&HashMap<String, String>>,
) -> anyhow::Result<()> {
    let renamed = renames
        .and_then(|renames| renames.get(&format!("{}.{}", ty.name, field.name)))
        .map(String::as_str);
    let accessor = crate::backends::swift::naming::text_value_accessor_name(&field.name, renamed);
    let collision = bridged.iter().filter(|other| other.name != field.name).any(|other| {
        crate::codegen::naming::public_field_name(crate::core::config::Language::Swift, &other.name, None) == accessor
    });
    if collision {
        anyhow::bail!(
            "Swift typed accessor `{accessor}()` for `{}.{}` collides with an existing public member",
            ty.name,
            field.name
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "text_value_accessors_tests.rs"]
mod tests;

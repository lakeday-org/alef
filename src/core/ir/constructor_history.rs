use super::{ApiSurface, DefaultValue, FieldDef, PrimitiveType, TypeDef, TypeRef};

impl TypeDef {
    /// The constructor prefix explicitly recorded on this type. ~keep
    #[must_use]
    pub fn legacy_constructor_fields(&self) -> &[String] {
        &self.version.legacy_constructor_fields
    }

    /// True only for a field added beyond an explicitly recorded constructor. ~keep
    #[must_use]
    pub fn is_constructor_extension(&self, field: &FieldDef) -> bool {
        !self.legacy_constructor_fields().is_empty() && !self.legacy_constructor_fields().contains(&field.name)
    }

    /// Resolve the supported extension default from Rust's extracted Default, never the field name. ~keep
    #[must_use]
    pub fn constructor_extension_default(&self, field: &FieldDef) -> Option<bool> {
        if !self.is_constructor_extension(field) || field.ty != TypeRef::Primitive(PrimitiveType::Bool) {
            return None;
        }
        match &field.typed_default {
            Some(DefaultValue::BoolLiteral(value)) => Some(*value),
            Some(DefaultValue::Empty) => Some(false),
            _ => None,
        }
    }
}

pub(crate) fn validate_constructor_history(api: &ApiSurface) -> anyhow::Result<()> {
    for typ in &api.types {
        if typ.legacy_constructor_fields().is_empty() {
            continue;
        }
        anyhow::ensure!(typ.has_default, "{} legacy constructor requires Rust Default", typ.name);
        anyhow::ensure!(
            !typ.is_opaque && !typ.has_private_fields,
            "{} legacy constructor metadata requires a transparent DTO",
            typ.name
        );
        anyhow::ensure!(
            !typ.methods
                .iter()
                .any(|method| matches!(method.name.as_str(), "new" | "new_with_fields")),
            "{} legacy constructor metadata conflicts with an explicit constructor",
            typ.name
        );
        let mut seen = std::collections::HashSet::new();
        for name in typ.legacy_constructor_fields() {
            anyhow::ensure!(seen.insert(name), "duplicate legacy constructor field `{name}`");
            anyhow::ensure!(
                typ.fields
                    .iter()
                    .any(|field| field.name == *name && !field.binding_excluded),
                "unknown legacy constructor field `{name}`"
            );
        }
        anyhow::ensure!(
            typ.fields
                .iter()
                .all(|field| field.binding_excluded || field.cfg.is_none()),
            "{} legacy constructor metadata requires unconditional fields",
            typ.name
        );
        for field in typ
            .fields
            .iter()
            .filter(|field| !field.binding_excluded && typ.is_constructor_extension(field))
        {
            anyhow::ensure!(
                typ.constructor_extension_default(field).is_some(),
                "{} legacy constructor extension `{}` requires a resolved Rust boolean default",
                typ.name,
                field.name
            );
        }
    }
    Ok(())
}

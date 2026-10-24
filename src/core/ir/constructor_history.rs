use super::{ApiSurface, DefaultValue, FieldDef, PrimitiveType, TypeDef, TypeRef};

impl TypeDef {
    /// The constructor prefix explicitly recorded on this type. ~keep
    #[must_use]
    pub fn legacy_constructor_fields(&self) -> &[String] {
        &self.version.legacy_constructor_fields
    }

    /// Additional explicitly recorded signatures, ordered from oldest to newest. ~keep
    #[must_use]
    pub fn constructor_history(&self) -> &[Vec<String>] {
        &self.version.constructor_history
    }

    /// Whether any constructor signature was explicitly recorded. ~keep
    #[must_use]
    pub fn has_recorded_constructors(&self) -> bool {
        !self.legacy_constructor_fields().is_empty() || !self.constructor_history().is_empty()
    }

    fn constructor_baseline_fields(&self) -> &[String] {
        if self.legacy_constructor_fields().is_empty() {
            self.constructor_history()
                .first()
                .map(Vec::as_slice)
                .unwrap_or_default()
        } else {
            self.legacy_constructor_fields()
        }
    }

    /// True only for a field added beyond an explicitly recorded constructor. ~keep
    #[must_use]
    pub fn is_constructor_extension(&self, field: &FieldDef) -> bool {
        !self.legacy_constructor_fields().is_empty() && !self.legacy_constructor_fields().contains(&field.name)
    }

    /// Include explicitly recorded Swift-only histories without changing legacy backend policies. ~keep
    #[must_use]
    pub fn is_recorded_constructor_extension(&self, field: &FieldDef) -> bool {
        let baseline = self.constructor_baseline_fields();
        self.has_recorded_constructors() && !baseline.contains(&field.name)
    }

    /// Preserve the boolean lookup exposed before typed constructor defaults were supported. ~keep
    #[must_use]
    pub fn constructor_extension_default(&self, field: &FieldDef) -> Option<bool> {
        if field.ty != TypeRef::Primitive(PrimitiveType::Bool) {
            return None;
        }
        match self.constructor_extension_value(field)? {
            DefaultValue::BoolLiteral(value) => Some(*value),
            DefaultValue::Empty => Some(false),
            _ => None,
        }
    }

    /// Resolve the supported extension default from Rust's extracted Default, never the field name. ~keep
    #[must_use]
    pub fn constructor_extension_value<'a>(&self, field: &'a FieldDef) -> Option<&'a DefaultValue> {
        if !self.is_constructor_extension(field) {
            return None;
        }
        Self::resolved_constructor_default(field)
    }

    /// Resolve omissions from any recorded Swift signature using the same exact Rust defaults. ~keep
    #[must_use]
    pub fn recorded_constructor_extension_value<'a>(&self, field: &'a FieldDef) -> Option<&'a DefaultValue> {
        if !self.is_recorded_constructor_extension(field) {
            return None;
        }
        Self::resolved_constructor_default(field)
    }

    fn resolved_constructor_default(field: &FieldDef) -> Option<&DefaultValue> {
        field.typed_default.as_ref().filter(|value| {
            !matches!(
                value,
                DefaultValue::Unresolved(_) | DefaultValue::FunctionCall(_) | DefaultValue::PublicFunctionCall(_)
            )
        })
    }
}

pub(crate) fn validate_constructor_history(api: &ApiSurface) -> anyhow::Result<()> {
    for typ in &api.types {
        if !typ.has_recorded_constructors() {
            continue;
        }
        validate_constructor_policy(typ)?;
        validate_legacy_fields(typ)?;
        validate_recorded_signatures(typ)?;
        validate_recorded_defaults(typ)?;
    }
    Ok(())
}

fn validate_constructor_policy(typ: &TypeDef) -> anyhow::Result<()> {
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
    anyhow::ensure!(
        typ.fields
            .iter()
            .all(|field| field.binding_excluded || field.cfg.is_none()),
        "{} legacy constructor metadata requires unconditional fields",
        typ.name
    );
    Ok(())
}

fn validate_legacy_fields(typ: &TypeDef) -> anyhow::Result<()> {
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
    Ok(())
}

fn validate_recorded_defaults(typ: &TypeDef) -> anyhow::Result<()> {
    for field in typ
        .fields
        .iter()
        .filter(|field| !field.binding_excluded && typ.is_recorded_constructor_extension(field))
    {
        anyhow::ensure!(
            typ.recorded_constructor_extension_value(field).is_some(),
            "{} legacy constructor extension `{}` of type `{:?}` requires a resolved Rust default",
            typ.name,
            field.name,
            field.ty
        );
    }
    Ok(())
}

fn validate_recorded_signatures(typ: &TypeDef) -> anyhow::Result<()> {
    let visible: Vec<_> = typ.fields.iter().filter(|field| !field.binding_excluded).collect();
    let mut previous = typ.legacy_constructor_fields();
    for (index, signature) in typ.constructor_history().iter().enumerate() {
        let context = format!("{} constructor history signature {}", typ.name, index + 1);
        let mut seen = std::collections::HashSet::new();
        let mut last_position = None;
        for name in signature {
            anyhow::ensure!(seen.insert(name), "{context} contains duplicate field `{name}`");
            let position = visible.iter().position(|field| field.name == *name);
            anyhow::ensure!(position.is_some(), "{context} contains unknown field `{name}`");
            anyhow::ensure!(last_position < position, "{context} reorders field `{name}`");
            last_position = position;
        }
        anyhow::ensure!(
            (signature.len() > previous.len() || (index == 0 && previous.is_empty()))
                && signature
                    .iter()
                    .filter(|name| previous.contains(name))
                    .eq(previous.iter()),
            "{context} must extend the previous recorded signature without duplicate or ambiguous fields"
        );
        anyhow::ensure!(
            signature.len() < visible.len(),
            "{context} duplicates the canonical constructor"
        );
        previous = signature;
    }
    Ok(())
}

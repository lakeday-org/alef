use crate::core::ir::{ErrorVariant, FieldDef};

// FRB mirrors and typed-error conversion must agree before display metadata can address fields by position. ~keep
pub(crate) fn visible(variant: &ErrorVariant) -> impl Iterator<Item = (usize, &FieldDef)> {
    variant
        .fields
        .iter()
        .enumerate()
        .filter(|(_, field)| !field.binding_excluded)
}

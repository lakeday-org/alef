use crate::core::ir::FieldDef;

pub(super) fn mirror_wire_rename(field: &FieldDef, attributes: &mut Vec<String>) {
    if field.serde_rename.is_none() || attributes.iter().any(|attribute| attribute.starts_with("serde(rename")) {
        return;
    }
    let wire_name = crate::codegen::naming::wire_field_name(&field.name, field.serde_rename.as_deref(), None);
    attributes.push(format!("serde(rename = {wire_name:?})"));
}

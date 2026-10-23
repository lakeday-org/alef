use crate::core::ir::FieldDef;

pub(super) fn field_alias_attributes(field: &FieldDef) -> Vec<String> {
    let mut aliases = Vec::new();
    let php_name = crate::codegen::naming::to_php_name(&field.name);
    if php_name != field.name {
        aliases.push(php_name);
    }
    if field
        .serde_rename
        .as_deref()
        .is_some_and(|wire_name| wire_name != field.name)
    {
        // ~keep PHP mirrors historically accepted the raw Rust key before explicit wire renames were restored.
        aliases.push(field.name.clone());
    }
    let wire_name = crate::codegen::naming::wire_field_name(&field.name, field.serde_rename.as_deref(), None);
    aliases.retain(|alias| alias != &wire_name);
    aliases.sort();
    aliases.dedup();
    aliases
        .into_iter()
        .map(|alias| format!("serde(alias = {alias:?})"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::field_alias_attributes;
    use crate::core::ir::FieldDef;

    #[test]
    fn php_wire_rename_aliases_preserve_legacy_keys_without_duplicate_canonical_names() {
        let cases = [
            ("call_type", None, vec!["serde(alias = \"callType\")"]),
            (
                "call_type",
                Some("type"),
                vec!["serde(alias = \"callType\")", "serde(alias = \"call_type\")"],
            ),
            ("call_type", Some("callType"), vec!["serde(alias = \"call_type\")"]),
            ("type", Some("type"), vec![]),
            ("kind", Some("category"), vec!["serde(alias = \"kind\")"]),
        ];
        assert_eq!(cases.len(), 5);
        for (name, wire_name, expected) in cases {
            let field = FieldDef {
                name: name.to_string(),
                serde_rename: wire_name.map(str::to_owned),
                ..Default::default()
            };
            assert_eq!(field_alias_attributes(&field), expected);
        }
    }
}

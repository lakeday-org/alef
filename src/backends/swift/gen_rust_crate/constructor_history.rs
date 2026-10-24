use super::type_bridge::{bridge_type, needs_json_bridge};
use crate::backends::swift::naming::{constructor_history_symbol, swift_rust_shim_ident};
use crate::core::ir::{FieldDef, TypeDef};
use heck::ToSnakeCase;

pub(super) struct ConstructorSignature<'a> {
    pub rust_name: String,
    pub fields: Vec<&'a FieldDef>,
    is_initializer: bool,
    swift_name: Option<&'static str>,
}

pub(super) fn signatures<'a>(owner: &'a TypeDef, fields: &[&'a FieldDef]) -> Vec<ConstructorSignature<'a>> {
    let has_legacy = !owner.legacy_constructor_fields().is_empty();
    let mut signatures = Vec::new();
    if has_legacy {
        signatures.push(ConstructorSignature {
            rust_name: "new".into(),
            fields: recorded_fields(owner.legacy_constructor_fields(), fields),
            is_initializer: true,
            swift_name: None,
        });
    }
    let recorded = owner
        .constructor_history()
        .iter()
        .map(|signature| recorded_fields(signature, fields));
    for (index, fields) in recorded.chain(std::iter::once(fields.to_vec())).enumerate() {
        let base = if has_legacy { "new_with_fields" } else { "new" };
        signatures.push(ConstructorSignature {
            rust_name: if index == 0 {
                base.into()
            } else {
                constructor_history_symbol(index)
            },
            fields,
            is_initializer: !has_legacy,
            swift_name: (has_legacy && index != 0).then_some("new_with_fields"),
        });
    }
    signatures
}

pub(super) fn validate(owner: &TypeDef, fields: &[&FieldDef]) -> anyhow::Result<()> {
    for name in owner
        .legacy_constructor_fields()
        .iter()
        .chain(owner.constructor_history().iter().flatten())
    {
        anyhow::ensure!(
            fields.iter().any(|field| field.name == *name),
            "Swift legacy constructor field {}.{name} is not exposed",
            owner.name
        );
    }
    for index in 1..=owner.constructor_history().len() {
        let symbol = constructor_history_symbol(index);
        let method_collision = owner
            .methods
            .iter()
            .any(|method| swift_rust_shim_ident(&method.name.to_snake_case()) == symbol);
        let field_collision = owner
            .fields
            .iter()
            .any(|field| swift_rust_shim_ident(&field.name.to_snake_case()) == symbol);
        anyhow::ensure!(
            !method_collision && !field_collision,
            "Swift DTO {} constructor history symbol `{symbol}` conflicts with an emitted method or getter",
            owner.name
        );
    }
    Ok(())
}

fn recorded_fields<'a>(signature: &[String], fields: &[&'a FieldDef]) -> Vec<&'a FieldDef> {
    signature
        .iter()
        .map(|name| {
            *fields
                .iter()
                .find(|field| &field.name == name)
                .expect("validated Swift constructor history field")
        })
        .collect()
}

pub(super) fn emit_externs(owner: &TypeDef, fields: &[&FieldDef]) -> String {
    let mut output = String::new();
    for signature in signatures(owner, fields) {
        let parameters = signature
            .fields
            .iter()
            .map(|field| {
                let bridge = bridge_type(&field.ty);
                let ty = if field.optional && !needs_json_bridge(&field.ty) {
                    format!("Option<{bridge}>")
                } else {
                    bridge
                };
                let name = swift_rust_shim_ident(&field.name.to_snake_case());
                format!("{name}: {ty}")
            })
            .collect::<Vec<_>>();
        if signature.is_initializer {
            output.push_str(&crate::backends::swift::template_env::render(
                "extern_init_attr.jinja",
                crate::alef_context! {},
            ));
        }
        output.push_str(&crate::backends::swift::template_env::render(
            "extern_fn_new.jinja",
            crate::alef_context! {
                params => parameters.join(", "), return_type => &owner.name,
                constructor_name => signature.rust_name,
                associated_type => if signature.is_initializer { None } else { Some(&owner.name) },
                swift_name => signature.swift_name,
            },
        ));
    }
    output
}

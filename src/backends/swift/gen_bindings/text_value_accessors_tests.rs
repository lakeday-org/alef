use super::*;
use crate::core::ir::{EnumDef, FieldDef, TypeDef};

fn reply_api() -> ApiSurface {
    let message = TypeDef {
        name: "Message".into(),
        rust_path: "sample::Message".into(),
        has_serde: true,
        fields: vec![
            FieldDef {
                name: "content".into(),
                ty: TypeRef::Named("Content".into()),
                optional: true,
                ..Default::default()
            },
            FieldDef {
                name: "summary_text".into(),
                ty: TypeRef::Named("Content".into()),
                ..Default::default()
            },
            FieldDef {
                name: "label".into(),
                ty: TypeRef::String,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    ApiSurface {
        crate_name: "sample".into(),
        types: vec![message],
        enums: vec![EnumDef {
            name: "Content".into(),
            rust_path: "sample::Content".into(),
            has_serde: true,
            serde_untagged: true,
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn emitted(api: &ApiSurface, text_types: &[&str], excluded: &[&str]) -> String {
    render(api, text_types, excluded, &HashMap::new()).unwrap()
}

fn render(
    api: &ApiSurface,
    configured: &[&str],
    excluded: &[&str],
    renames: &HashMap<String, String>,
) -> anyhow::Result<String> {
    let configured = configured.iter().map(|name| name.to_string()).collect::<Vec<_>>();
    let excluded = excluded.iter().map(|field| field.to_string()).collect();
    let mut out = String::new();
    emit(
        api,
        AccessorConfig {
            text_types: &configured,
            rename_fields: Some(renames),
        },
        &HashSet::new(),
        &excluded,
        &HashSet::new(),
        &mut out,
    )?;
    Ok(out)
}

#[test]
fn text_fields_decode_the_fallible_lossless_companion() {
    let out = emitted(&reply_api(), &["Content"], &[]);
    assert!(out.starts_with("extension RustBridge.MessageRef {"), "{out}");
    assert!(out.contains("public func contentValue() throws -> Content? {"), "{out}");
    assert!(
        out.contains("try JSONDecoder().decode(Content?.self, from: Data(__alef_json_content().toString().utf8))"),
        "{out}"
    );
    assert!(
        out.contains("public func summaryTextValue() throws -> Content {"),
        "{out}"
    );
    assert!(
        out.contains("try JSONDecoder().decode(Content.self, from: Data(__alef_json_summary_text().toString().utf8))"),
        "{out}"
    );
    assert!(!out.contains("labelValue"), "{out}");
}

#[test]
fn unconfigured_text_types_do_not_add_accessors() {
    assert_eq!(emitted(&reply_api(), &[], &[]), "");
}

#[test]
fn excluded_field_has_no_accessor() {
    let out = emitted(&reply_api(), &["Content"], &["Message.content"]);
    assert!(!out.contains("contentValue"), "{out}");
    assert!(out.contains("summaryTextValue"), "{out}");
}

#[test]
fn unrelated_fields_do_not_add_an_extension() {
    let mut api = reply_api();
    api.types[0].fields.retain(|field| field.name == "label");
    assert_eq!(emitted(&api, &["Content"], &[]), "");
}

fn extra_field(name: &str) -> ApiSurface {
    let mut api = reply_api();
    api.types[0].fields.push(FieldDef {
        name: name.into(),
        ty: TypeRef::String,
        ..Default::default()
    });
    api
}

#[test]
fn former_public_json_suffix_is_not_the_internal_companion() {
    for name in ["content_json", "contentJson"] {
        assert!(render(&extra_field(name), &["Content"], &[], &HashMap::new()).is_ok());
    }
}

#[test]
fn actual_bridge_getter_collision_is_rejected() {
    for name in ["content_value", "contentValue"] {
        let error = render(&extra_field(name), &["Content"], &[], &HashMap::new()).unwrap_err();
        assert!(error.to_string().contains("contentValue()"), "{error}");
    }
}

#[test]
fn excluded_bridge_getter_does_not_collide() {
    let out = emitted(&extra_field("content_value"), &["Content"], &["Message.content_value"]);
    assert!(out.contains("contentValue()"), "{out}");
}

#[test]
fn similarly_named_getter_does_not_collide() {
    assert!(emitted(&extra_field("content_values"), &["Content"], &[]).contains("contentValue()"));
}

#[test]
fn serde_capability_controls_public_typed_accessor() {
    for parent in [false, true] {
        for enumeration in [false, true] {
            for skipped in [false, true] {
                let mut api = reply_api();
                api.types[0].has_serde = parent;
                api.enums[0].has_serde = enumeration;
                api.types[0].fields[0].serde_skip = skipped;
                assert_eq!(
                    emitted(&api, &["Content"], &[]).contains("func contentValue()"),
                    parent && enumeration && !skipped
                );
            }
        }
    }
}

#[test]
fn global_method_shim_does_not_share_bridge_getter_scope() {
    let mut api = reply_api();
    api.types[0].methods.push(crate::core::ir::MethodDef {
        name: "content_value".into(),
        ..Default::default()
    });
    assert!(emitted(&api, &["Content"], &[]).contains("contentValue()"));
}

#[test]
fn dto_property_rename_does_not_share_bridge_getter_scope() {
    let renames = HashMap::from([("Message.other".into(), "content_value".into())]);
    let out = render(&extra_field("other"), &["Content"], &[], &renames).unwrap();
    assert!(out.contains("contentValue()"), "{out}");
}

#[test]
fn own_accessor_rename_uses_canonical_public_policy() {
    for (rename, expected) in [
        ("body_payload", "bodyPayloadValue"),
        ("URL", "urlValue"),
        ("class", "classValue"),
    ] {
        let renames = HashMap::from([("Message.content".into(), rename.into())]);
        let out = render(&reply_api(), &["Content"], &[], &renames).unwrap();
        assert!(out.contains(&format!("func {expected}()")), "{rename}: {out}");
        assert!(out.contains("__alef_json_content()"), "{out}");
    }
}

#[test]
fn configured_dto_rename_does_not_hide_actual_raw_getter_collision() {
    let renames = HashMap::from([("Message.content_value".into(), "other".into())]);
    let error = render(&extra_field("content_value"), &["Content"], &[], &renames).unwrap_err();
    assert!(error.to_string().contains("contentValue"), "{error}");
}

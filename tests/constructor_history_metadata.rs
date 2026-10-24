use alef::core::ir::{ApiSurface, VersionAnnotation};

fn extract(annotation: &str) -> anyhow::Result<ApiSurface> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("lib.rs");
    std::fs::write(
        &path,
        format!(
            r#"
#[derive(Clone, serde::Serialize, serde::Deserialize)]
{annotation}
pub struct Options {{ pub url: String, pub enabled: bool, pub depth: u32, pub detailed: bool }}
impl Default for Options {{
    fn default() -> Self {{ Self {{ url: "sample".to_owned(), enabled: true, depth: 7, detailed: false }} }}
}}
"#
        ),
    )?;
    let api = alef::extract::extractor::extract(&[&path], "fixture", "1.0.0", None)?;
    assert_eq!(api.types.len(), 1);
    assert_eq!(api.types[0].fields.len(), 4);
    Ok(api)
}

#[test]
fn additional_history_round_trips_without_replacing_the_legacy_signature() {
    let api = extract(
        r#"#[cfg_attr(alef, alef(
        legacy_constructor_fields = ["url"], constructor_history = [["url", "enabled"]]
    ))]"#,
    )
    .unwrap();
    let owner = &api.types[0];
    assert_eq!(owner.legacy_constructor_fields(), ["url"]);
    assert_eq!(
        owner.constructor_history(),
        [vec!["url".to_owned(), "enabled".to_owned()]]
    );
    let json = serde_json::to_value(&owner.version).unwrap();
    assert_eq!(json["constructor_history"], serde_json::json!([["url", "enabled"]]));
    let restored: VersionAnnotation = serde_json::from_value(json).unwrap();
    assert_eq!(restored, owner.version);
}

#[test]
fn history_only_metadata_uses_exact_resolved_defaults_for_omitted_fields() {
    let api = extract(r#"#[alef(constructor_history = [["url"], ["url", "enabled"]])]"#).unwrap();
    let owner = &api.types[0];
    assert!(owner.legacy_constructor_fields().is_empty());
    assert!(owner.has_recorded_constructors());
    assert!(!owner.is_constructor_extension(&owner.fields[0]));
    assert_eq!(owner.constructor_extension_default(&owner.fields[1]), None);
    assert_eq!(
        owner.recorded_constructor_extension_value(&owner.fields[1]),
        Some(&alef::core::ir::DefaultValue::BoolLiteral(true))
    );
    assert_eq!(
        owner.recorded_constructor_extension_value(&owner.fields[2]),
        Some(&alef::core::ir::DefaultValue::IntLiteral(7))
    );
    assert_eq!(
        owner.recorded_constructor_extension_value(&owner.fields[3]),
        Some(&alef::core::ir::DefaultValue::BoolLiteral(false))
    );
}

#[test]
fn old_ir_remains_readable_and_absent_history_is_not_serialized() {
    for json in [
        serde_json::json!({}),
        serde_json::json!({"legacy_constructor_fields": ["url"]}),
    ] {
        let version: VersionAnnotation = serde_json::from_value(json).unwrap();
        assert!(version.constructor_history.is_empty());
        assert!(
            serde_json::to_value(version)
                .unwrap()
                .get("constructor_history")
                .is_none()
        );
    }
}

#[test]
fn invalid_explicit_signatures_fail_with_the_owner_and_specific_reason() {
    let cases = [
        (r#"[["missing"]]"#, "unknown field `missing`"),
        (r#"[["url", "url"]]"#, "duplicate field `url`"),
        (r#"[["enabled", "url"]]"#, "reorders field `url`"),
        (r#"[["url"], ["url"]]"#, "duplicate or ambiguous"),
        (r#"[["url"], ["enabled", "depth"]]"#, "duplicate or ambiguous"),
        (
            r#"[["url", "enabled", "depth", "detailed"]]"#,
            "duplicates the canonical",
        ),
    ];
    assert_eq!(cases.len(), 6);
    for (history, reason) in cases {
        let annotation = format!("#[alef(constructor_history = {history})]");
        let error = extract(&annotation).expect_err(history).to_string();
        assert!(
            error.contains("Options constructor history signature"),
            "{history}: {error}"
        );
        assert!(error.contains(reason), "{history}: {error}");
    }
}

#[test]
fn an_explicit_first_zero_argument_history_requires_defaults_for_every_field() {
    let api = extract(r#"#[alef(constructor_history = [[], ["url"]])]"#).unwrap();
    let owner = &api.types[0];
    assert_eq!(owner.constructor_history().len(), 2);
    assert!(owner.constructor_history()[0].is_empty());
    assert!(
        owner
            .fields
            .iter()
            .all(|field| owner.recorded_constructor_extension_value(field).is_some())
    );
    let error = extract(r#"#[alef(legacy_constructor_fields = ["url"], constructor_history = [[]])]"#)
        .unwrap_err()
        .to_string();
    assert!(error.contains("must extend the previous recorded signature"), "{error}");
}

#[test]
fn a_zero_argument_history_rejects_an_unresolved_omitted_field_default() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lib.rs");
    std::fs::write(
        &path,
        r#"
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[alef(constructor_history = [[]])]
pub struct Options { pub label: String }
impl Default for Options {
    fn default() -> Self { Self { label: unresolved_label() } }
}
"#,
    )
    .unwrap();
    let error = alef::extract::extractor::extract(&[&path], "fixture", "1.0.0", None)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("Options legacy constructor extension `label`"),
        "{error}"
    );
    assert!(error.contains("requires a resolved Rust default"), "{error}");
}

#[test]
fn malformed_history_annotations_are_rejected_instead_of_silently_ignored() {
    let cases = [
        ("[]", "must not be empty"),
        (r#"["url"]"#, "signatures must be arrays"),
        ("[[false]]", "must contain string field names"),
        ("false", "must be an array of field-name arrays"),
    ];
    assert_eq!(cases.len(), 4);
    for (history, reason) in cases {
        let error = extract(&format!("#[alef(constructor_history = {history})]"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("constructor_history"), "{history}: {error}");
        assert!(error.contains(reason), "{history}: {error}");
    }
    let error = extract(r#"#[alef(constructor_history = [["url"]], constructor_history = [["url"]])]"#)
        .unwrap_err()
        .to_string();
    assert!(error.contains("duplicate constructor_history annotation"), "{error}");
}

#[test]
fn additional_history_cannot_reorder_an_existing_legacy_signature() {
    let error = extract(
        r#"#[alef(
        legacy_constructor_fields = ["enabled", "url"],
        constructor_history = [["url", "enabled", "depth"]]
    )]"#,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("Options constructor history signature 1"), "{error}");
    assert!(error.contains("duplicate or ambiguous"), "{error}");
    let legacy_only = extract(r#"#[alef(legacy_constructor_fields = ["enabled", "url"])]"#).unwrap();
    assert_eq!(legacy_only.types[0].legacy_constructor_fields(), ["enabled", "url"]);
}

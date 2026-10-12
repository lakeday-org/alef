use alef::core::backend::Backend;
use alef::core::config::{ResolvedCrateConfig, new_config::NewAlefConfig};
use alef::core::ir::ApiSurface;

fn extract(annotation: &str) -> anyhow::Result<ApiSurface> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("lib.rs");
    std::fs::write(
        &source,
        format!(
            r#"
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
{annotation}
pub struct Options {{
    pub skip_images: bool,
    pub max_depth: Option<usize>,
    pub keep_hidden: bool,
}}
"#
        ),
    )?;
    alef::extract::extractor::extract(&[&source], "sample", "1.0.0", None)
}

fn annotation() -> &'static str {
    r#"#[cfg_attr(alef, alef(legacy_constructor_fields = ["skip_images", "max_depth"]))]"#
}

fn config() -> ResolvedCrateConfig {
    let raw: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["python", "java", "swift", "dart"]
[[crates]]
name = "sample"
sources = ["src/lib.rs"]
"#,
    )
    .expect("valid config");
    raw.resolve().expect("resolved config").remove(0)
}

fn render(backend: &dyn Backend) -> String {
    let api = extract(annotation()).expect("annotated API");
    let files = backend.generate_bindings(&api, &config()).expect("binding source");
    assert!(!files.is_empty(), "the backend must emit actual files");
    files
        .into_iter()
        .map(|file| file.content)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn should_extract_legacy_constructor_fields_and_omit_absent_metadata() {
    let api = extract(annotation()).expect("valid annotated API");
    let value = serde_json::to_value(&api.types[0]).expect("IR JSON");
    assert_eq!(
        value["version"]["legacy_constructor_fields"],
        serde_json::json!(["skip_images", "max_depth"])
    );
    let plain = extract("").expect("unannotated API");
    assert!(
        serde_json::to_value(&plain.types[0])
            .expect("IR JSON")
            .get("version")
            .expect("version metadata")
            .get("legacy_constructor_fields")
            .is_none()
    );
}

#[test]
fn should_reject_unknown_legacy_fields_with_the_field_name() {
    let result = extract(r#"#[alef(legacy_constructor_fields = ["unknown"])]"#);
    let error = result.expect_err("unknown field must fail extraction").to_string();
    assert!(error.contains("unknown legacy constructor field `unknown`"), "{error}");
}

#[test]
fn should_reject_duplicate_legacy_fields_with_the_field_name() {
    let result = extract(r#"#[alef(legacy_constructor_fields = ["skip_images", "skip_images"])]"#);
    let error = result.expect_err("duplicate field must fail extraction").to_string();
    assert!(
        error.contains("duplicate legacy constructor field `skip_images`"),
        "{error}"
    );
}

#[test]
fn should_keep_native_python_positional_parameters_before_new_keyword_only_fields() {
    let source = render(&alef::backends::pyo3::Pyo3Backend);
    assert!(
        source.contains("max_depth=None, *, keep_hidden=Self::default().keep_hidden"),
        "{source}"
    );
}

#[test]
fn should_give_new_dart_named_constructor_fields_the_known_rust_default() {
    let source = render(&alef::backends::dart::DartBackend);
    assert!(
        source.contains("#[frb(default = false)]\n    pub keep_hidden: bool"),
        "{source}"
    );
}

#[test]
fn should_emit_java_legacy_constructor_delegating_with_default_for_the_added_field() {
    let source = render(&alef::backends::java::JavaBackend);
    assert!(
        source.contains("public Options(boolean skipImages, Long maxDepth)"),
        "{source}"
    );
    assert!(source.contains("this(skipImages, maxDepth, false);"), "{source}");
}

#[test]
fn should_keep_swift_legacy_init_and_emit_a_separate_complete_constructor() {
    let source = render(&alef::backends::swift::SwiftBackend);
    assert!(
        source.contains("fn new(skip_images: bool, max_depth: Option<usize>) -> Options;"),
        "{source}"
    );
    assert!(source.contains("fn new_with_fields("), "{source}");
    assert!(source.contains("pub fn new_with_fields("), "{source}");
}

#[test]
fn all_legacy_fields_keep_the_java_canonical_constructor_without_a_duplicate() {
    let api = extract(r#"#[alef(legacy_constructor_fields = ["skip_images", "max_depth", "keep_hidden"])]"#).unwrap();
    let files = alef::backends::java::JavaBackend
        .generate_bindings(&api, &config())
        .unwrap();
    let record = files.iter().find(|file| file.path.ends_with("Options.java")).unwrap();
    assert!(
        !record
            .content
            .contains("public Options(boolean skipImages, Long maxDepth, boolean keepHidden)"),
        "redundant canonical constructor: {}",
        record.content
    );
}

#[test]
fn an_excluded_swift_legacy_parameter_returns_a_contextual_generation_error() {
    let api = extract(annotation()).unwrap();
    let mut config = config();
    config
        .swift
        .get_or_insert_with(Default::default)
        .exclude_fields
        .push("Options.max_depth".into());
    let error = alef::backends::swift::SwiftBackend
        .generate_bindings(&api, &config)
        .expect_err("excluding a declared ABI argument must be rejected")
        .to_string();
    assert!(
        error.contains("legacy constructor") && error.contains("Options.max_depth"),
        "{error}"
    );
}

#[test]
fn reordered_legacy_fields_that_collide_with_the_java_canonical_signature_are_rejected() {
    let api = extract(r#"#[alef(legacy_constructor_fields = ["keep_hidden", "max_depth", "skip_images"])]"#).unwrap();
    let error = alef::backends::java::JavaBackend
        .generate_bindings(&api, &config())
        .expect_err("canonical signature cannot preserve a different parameter order")
        .to_string();
    assert!(
        error.contains("legacy constructor order") && error.contains("Options"),
        "{error}"
    );
}

#[test]
fn a_hidden_java_legacy_parameter_returns_a_contextual_generation_error() {
    let mut api = extract(annotation()).unwrap();
    api.types[0].fields[1].binding_excluded = true;
    let error = alef::backends::java::JavaBackend
        .generate_bindings(&api, &config())
        .expect_err("hidden ABI argument must be rejected")
        .to_string();
    assert!(
        error.contains("legacy constructor") && error.contains("Options.max_depth"),
        "{error}"
    );
}

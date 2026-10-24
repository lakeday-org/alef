use alef::core::backend::{Backend, GeneratedFile};
use alef::core::config::{NewAlefConfig, ResolvedCrateConfig};
use alef::core::ir::ApiSurface;

#[path = "swift_constructor_history/native.rs"]
mod native;

fn fixture_source(legacy: bool, arity: usize, history: &[Vec<&str>]) -> String {
    let mut environment = minijinja::Environment::new();
    alef::template::configure_env(&mut environment);
    environment.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    environment
        .add_template("core", include_str!("fixtures/swift_constructor_history/core.rs.jinja"))
        .unwrap();
    environment
        .get_template("core")
        .unwrap()
        .render(alef::alef_context! { legacy => legacy, arity => arity, history => history })
        .unwrap()
}

fn config() -> ResolvedCrateConfig {
    let raw: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["swift"]
[[crates]]
name = "fixture"
sources = ["src/lib.rs"]
"#,
    )
    .unwrap();
    raw.resolve().unwrap().remove(0)
}

fn api_and_files(source: &str) -> (ApiSurface, Vec<GeneratedFile>) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lib.rs");
    std::fs::write(&path, source).unwrap();
    let api = alef::extract::extractor::extract(&[&path], "fixture", "1.0.0", None).unwrap();
    assert_eq!(api.types.len(), 1);
    let files = alef::backends::swift::SwiftBackend
        .generate_bindings(&api, &config())
        .unwrap();
    assert!(files.len() >= 2);
    (api, files)
}

fn file<'a>(files: &'a [GeneratedFile], suffix: &str) -> &'a str {
    &files
        .iter()
        .find(|file| file.path.ends_with(suffix))
        .expect(suffix)
        .content
}

fn history(legacy: bool) -> Vec<Vec<&'static str>> {
    if legacy {
        vec![vec!["count", "enabled"]]
    } else {
        vec![vec!["count"], vec!["count", "enabled"]]
    }
}

#[test]
fn exact_explicit_initializers_and_factory_signatures_survive_multiple_extensions() {
    for legacy in [true, false] {
        let source = fixture_source(legacy, 6, &history(legacy));
        let (_, files) = api_and_files(&source);
        let facade = file(&files, "Sources/Fixture/Fixture.swift");
        for signature in [
            "public init(count: UInt32 = 7)",
            "public init(count: UInt32 = 7, enabled: Bool = true)",
        ] {
            assert!(facade.contains(signature), "{signature}\n{facade}");
        }
        assert_eq!(facade.matches("    public init(count:").count(), 3);
        let bridge = file(&files, "rust/src/lib.rs");
        let original = if legacy { "new_with_fields" } else { "new" };
        let original_arity = if legacy {
            "count: u32, enabled: bool"
        } else {
            "count: u32"
        };
        assert!(
            bridge.contains(&format!("fn {original}({original_arity}) -> Options;")),
            "{bridge}"
        );
        assert!(bridge.contains("fn __alef_constructor_history_1("), "{bridge}");
        assert_eq!(
            bridge.matches("swift_name = \"new_with_fields\"").count(),
            usize::from(legacy)
        );
        assert_eq!(
            bridge.matches("#[swift_bridge(init)]").count(),
            if legacy { 1 } else { 3 }
        );
    }
}

#[test]
fn unrecorded_intermediate_signatures_are_not_inferred() {
    let (_, files) = api_and_files(&fixture_source(true, 6, &[]));
    let facade = file(&files, "Sources/Fixture/Fixture.swift");
    assert_eq!(facade.matches("    public init(count:").count(), 2);
    assert!(!facade.contains("public init(count: UInt32 = 7, enabled: Bool = true)"));
    assert!(!file(&files, "rust/src/lib.rs").contains("__alef_constructor_history_"));
}

#[test]
fn recorded_bridge_fields_are_validated_against_the_actual_swift_surface() {
    let (api, _) = api_and_files(&fixture_source(true, 6, &history(true)));
    let raw: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["swift"]
[[crates]]
name = "fixture"
sources = ["src/lib.rs"]
[crates.swift]
exclude_fields = ["Options.enabled"]
"#,
    )
    .unwrap();
    let error = alef::backends::swift::SwiftBackend
        .generate_bindings(&api, &raw.resolve().unwrap().remove(0))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("Options")
            && error.contains("enabled")
            && (error.contains("not exposed") || error.contains("unavailable")),
        "{error}"
    );
}

#[test]
fn ordinary_fields_cannot_collide_with_reserved_constructor_shims() {
    let source = fixture_source(true, 6, &history(true)).replace("include_labels", "new_with_fields_history_0");
    let (_, files) = api_and_files(&source);
    let bridge = file(&files, "rust/src/lib.rs");
    assert!(bridge.contains("fn new_with_fields_history_0(&self) -> bool;"));
    assert!(bridge.contains("fn __alef_constructor_history_1("));
    assert!(!bridge.contains("fn new_with_fields_history_0(count:"));
}

#[test]
fn swift_only_history_leaves_other_backend_constructors_unchanged() {
    let (mut api, _) = api_and_files(&fixture_source(false, 6, &history(false)));
    for backend in [
        &alef::backends::pyo3::Pyo3Backend as &dyn Backend,
        &alef::backends::java::JavaBackend as &dyn Backend,
    ] {
        let recorded = backend.generate_bindings(&api, &config()).unwrap();
        let history = std::mem::take(&mut api.types[0].version.constructor_history);
        let unrecorded = backend.generate_bindings(&api, &config()).unwrap();
        api.types[0].version.constructor_history = history;
        assert!(!recorded.is_empty());
        let contents = |files: Vec<GeneratedFile>| {
            files
                .into_iter()
                .map(|file| (file.path, file.content))
                .collect::<Vec<_>>()
        };
        assert_eq!(contents(recorded), contents(unrecorded));
    }
}

use super::*;

#[test]
fn resolve_rejects_unknown_skip_languages_in_adapter() {
    let cfg: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["python"]

[[crates]]
name = "sample_router"
sources = ["src/lib.rs"]

[[crates.adapters]]
name = "stream_data"
pattern = "streaming"
core_path = "my_crate::stream_data"
skip_languages = ["wasm32"]
"#,
    )
    .unwrap();
    let err = cfg.resolve().unwrap_err();
    assert!(
        matches!(&err, ResolveError::InvalidConfig(msg) if msg.contains("wasm32")),
        "expected InvalidConfig error mentioning the bad name, got: {err:?}"
    );
}

#[test]
fn resolve_accepts_valid_skip_languages_in_adapter() {
    let cfg: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["python"]

[[crates]]
name = "sample_router"
sources = ["src/lib.rs"]

[[crates.adapters]]
name = "stream_data"
pattern = "streaming"
core_path = "my_crate::stream_data"
skip_languages = ["wasm", "kotlin"]
"#,
    )
    .unwrap();
    let resolved = cfg.resolve().expect("valid skip_languages should not fail");
    assert_eq!(resolved[0].adapters[0].skip_languages, vec!["wasm", "kotlin"]);
}

#[test]
fn resolve_rejects_unknown_language_in_registration_variant() {
    let cfg: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["python"]

[[crates]]
name = "sample_router"
sources = ["src/lib.rs"]

[[crates.handler_contracts]]
trait_name = "Handler"
dispatch_method = "call"

[[crates.services]]
owner_type = "App"

[[crates.services.registrations]]
method = "add_route"
callback_param = "handler"
callback_bound = "IntoHandler"
callback_contract = "Handler"

[[crates.services.registrations.variants]]
name = "get"
fixed = { method = "GET" }

[crates.services.registrations.variants.languages.knotlin]
method_prefix = "Map"
"#,
    )
    .unwrap();
    let err = cfg.resolve().unwrap_err();
    assert!(
        matches!(&err, ResolveError::InvalidConfig(msg) if msg.contains("knotlin")),
        "expected InvalidConfig error mentioning the bad name, got: {err:?}"
    );
}

#[test]
fn resolve_accepts_valid_language_in_registration_variant() {
    let cfg: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["python"]

[[crates]]
name = "sample_router"
sources = ["src/lib.rs"]

[[crates.handler_contracts]]
trait_name = "Handler"
dispatch_method = "call"

[[crates.services]]
owner_type = "App"

[[crates.services.registrations]]
method = "add_route"
callback_param = "handler"
callback_bound = "IntoHandler"
callback_contract = "Handler"

[[crates.services.registrations.variants]]
name = "get"
fixed = { method = "GET" }

[crates.services.registrations.variants.languages.kotlin]
method_prefix = "Map"
"#,
    )
    .unwrap();
    let resolved = cfg.resolve().expect("valid variant language should not fail");
    assert!(
        resolved[0].services[0].registrations[0].variants[0]
            .languages
            .contains_key("kotlin")
    );
}

#[test]
fn resolve_rejects_unknown_language_in_trait_bridge_exclude_languages() {
    let cfg: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["python"]

[[crates]]
name = "sample_router"
sources = ["src/lib.rs"]

[[crates.trait_bridges]]
trait_name = "OcrBackend"
exclude_languages = ["wasm32"]
"#,
    )
    .unwrap();
    let err = cfg.resolve().unwrap_err();
    assert!(
        matches!(&err, ResolveError::InvalidConfig(msg) if msg.contains("wasm32")),
        "expected InvalidConfig error mentioning the bad name, got: {err:?}"
    );
}

#[test]
fn resolve_accepts_valid_trait_bridge_exclude_languages() {
    let cfg: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["python"]

[[crates]]
name = "sample_router"
sources = ["src/lib.rs"]

[[crates.trait_bridges]]
trait_name = "OcrBackend"
exclude_languages = ["wasm", "elixir"]
"#,
    )
    .unwrap();
    let resolved = cfg.resolve().expect("valid exclude_languages should not fail");
    assert_eq!(resolved[0].trait_bridges[0].exclude_languages, vec!["wasm", "elixir"]);
}

#[test]
fn resolve_rejects_unknown_language_in_callback_unsupported_mode() {
    let cfg: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["php"]

[[crates]]
name = "sample_router"
sources = ["src/lib.rs"]

[[crates.trait_bridges]]
trait_name = "OcrBackend"
exclude_languages = ["php-zts:callbacks"]
"#,
    )
    .unwrap();
    let err = cfg.resolve().unwrap_err();
    assert!(
        matches!(&err, ResolveError::InvalidConfig(msg) if msg.contains("php-zts:callbacks") && msg.contains("exclude_languages")),
        "expected InvalidConfig naming the field and bad language, got: {err:?}"
    );
}

#[test]
fn resolve_rejects_callback_unsupported_mode_for_non_php_language() {
    let cfg: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["python"]

[[crates]]
name = "sample_router"
sources = ["src/lib.rs"]

[[crates.trait_bridges]]
trait_name = "OcrBackend"
exclude_languages = ["python:callbacks"]
"#,
    )
    .unwrap();
    let err = cfg.resolve().unwrap_err();
    assert!(
        matches!(&err, ResolveError::InvalidConfig(msg) if msg.contains("python:callbacks") && msg.contains("exclude_languages")),
        "expected InvalidConfig naming the unsupported mode and field, got: {err:?}"
    );
}

/// Every backend answers to its own name as well as its language's, and
/// `bridge_targets_language` documents exactly that -- but resolution used to reject the
/// backend spelling, so the documented form was unusable. ~keep
#[test]
fn resolve_accepts_a_backend_spelling_in_trait_bridge_exclude_languages() {
    for spelling in ["pyo3", "napi", "magnus", "rustler", "extendr"] {
        let cfg: NewAlefConfig = toml::from_str(&format!(
            r#"
[workspace]
languages = ["python"]

[[crates]]
name = "sample_router"
sources = ["src/lib.rs"]

[[crates.trait_bridges]]
trait_name = "OcrBackend"
exclude_languages = ["{spelling}"]
"#
        ))
        .unwrap();
        let resolved = cfg
            .resolve()
            .unwrap_or_else(|e| panic!("`{spelling}` is a documented backend spelling: {e:?}"));
        assert!(!resolved[0].trait_bridges[0].is_active_for(spelling));
    }
}

#[test]
fn rejected_exclude_language_error_lists_both_name_families() {
    let cfg: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["python"]

[[crates]]
name = "sample_router"
sources = ["src/lib.rs"]

[[crates.trait_bridges]]
trait_name = "OcrBackend"
exclude_languages = ["nodejs"]
"#,
    )
    .unwrap();
    let err = cfg.resolve().unwrap_err();
    let ResolveError::InvalidConfig(message) = &err else {
        panic!("expected InvalidConfig, got {err:?}");
    };
    assert!(
        message.contains("`nodejs`"),
        "must quote the offending entry: {message}"
    );
    assert!(
        message.contains("kotlin_android") && message.contains("pyo3"),
        "must list the language names and the backend spellings: {message}"
    );
}

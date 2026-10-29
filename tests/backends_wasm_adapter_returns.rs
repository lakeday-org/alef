use alef::backends::wasm::WasmBackend;
use alef::core::backend::Backend;
use alef::core::config::{AdapterConfig, AdapterPattern, NewAlefConfig, ResolvedCrateConfig};
use alef::core::ir::ApiSurface;
use alef::snippets::validators::run_command_streams;
use std::path::Path;

#[path = "support/mise_task.rs"]
mod mise_task;

const CORE: &str = include_str!("fixtures/wasm_adapter_returns/core.rs");

fn configuration(prefix: &str) -> ResolvedCrateConfig {
    let source = format!(
        "[workspace]\nlanguages = [\"wasm\"]\n[[crates]]\nname = \"adapter-sample\"\nsources = [\"src/lib.rs\"]\n[crates.wasm]\ntype_prefix = {prefix:?}\n"
    );
    let parsed: NewAlefConfig = toml::from_str(&source).unwrap();
    let mut config = parsed.resolve().unwrap().remove(0);
    config.adapters = [
        ("reply", "Reply"),
        ("optional_reply", "Option<Reply>"),
        ("replies", "Vec<Reply>"),
        ("optional_replies", "Option<Vec<Reply>>"),
        ("sparse_replies", "Vec<Option<Reply>>"),
        ("flush", "()"),
        ("bytes", "bytes::Bytes"),
        ("sync_reply", "Reply"),
    ]
    .into_iter()
    .map(|(name, returns)| AdapterConfig {
        name: name.into(),
        owner_type: Some("Client".into()),
        pattern: if name == "sync_reply" {
            AdapterPattern::SyncFunction
        } else {
            AdapterPattern::AsyncMethod
        },
        core_path: if name == "sync_reply" {
            "adapter_sample::reply_sync".into()
        } else {
            name.into()
        },
        returns: Some(returns.into()),
        error_type: Some("ApiError".into()),
        params: vec![],
        item_type: None,
        gil_release: false,
        trait_name: None,
        trait_method: None,
        detect_async: false,
        request_type: None,
        skip_languages: vec![],
    })
    .collect();
    config
}

fn generate(root: &Path, config: &ResolvedCrateConfig) -> String {
    let input = root.join("lib.rs");
    std::fs::write(&input, CORE).unwrap();
    let mut api: ApiSurface = alef::extract::extractor::extract(&[&input], "adapter-sample", "1.0.0", None).unwrap();
    for ty in &mut api.types {
        if ty.name == "Client" {
            ty.is_opaque = true;
        }
        for method in &mut ty.methods {
            method.sanitized = true;
        }
    }
    for function in &mut api.functions {
        function.sanitized = true;
    }
    WasmBackend
        .generate_bindings(&api, config)
        .unwrap()
        .into_iter()
        .find(|file| file.path.ends_with("src/lib.rs"))
        .unwrap()
        .content
}

#[test]
fn adapters_map_named_returns_through_the_configured_wasm_wrapper() {
    let root = tempfile::tempdir().unwrap();
    let source = generate(root.path(), &configuration("Bridge"));
    assert_eq!(source.matches(".map(BridgeReply::from)").count(), 5, "{source}");
    assert!(source.contains("value.into_iter().map(BridgeReply::from)"), "{source}");
    assert!(!source.contains(".map(Reply::from)"), "{source}");
    assert!(source.contains("result.to_vec()"), "{source}");
    assert!(source.contains("serde_wasm_bindgen::to_value(&(value))"), "{source}");
    assert!(source.contains(".map_err(alef_wasm_err)"), "{source}");
    syn::parse_file(&source).unwrap();
}

#[test]
fn configured_jsvalue_overrides_keep_serialization_instead_of_wrapper_construction() {
    let root = tempfile::tempdir().unwrap();
    let mut config = configuration("Wasm");
    config
        .wasm
        .as_mut()
        .unwrap()
        .type_overrides
        .insert("Reply".into(), "JsValue".into());
    let source = generate(root.path(), &config);
    assert!(source.contains("serde_wasm_bindgen::to_value(&(value))"), "{source}");
    assert!(!source.contains("JsValue::from(result)"), "{source}");
}

fn write_native_fixture(root: &Path, source: &str) {
    std::fs::create_dir_all(root.join("core/src")).unwrap();
    std::fs::create_dir_all(root.join("binding/src")).unwrap();
    for (path, contents) in [
        ("core/src/lib.rs", CORE),
        (
            "core/Cargo.toml",
            include_str!("fixtures/wasm_adapter_returns/core_Cargo.toml"),
        ),
        ("binding/src/lib.rs", source),
        (
            "binding/Cargo.toml",
            include_str!("fixtures/wasm_adapter_returns/bridge_Cargo.toml"),
        ),
        ("mise.toml", include_str!("fixtures/wasm_adapter_returns/mise.toml")),
    ] {
        std::fs::write(root.join(path), contents).unwrap();
    }
}

fn lint(root: &Path) -> alef::snippets::validators::CapturedStreams {
    let mut command = mise_task::mise_run(root, "lint");
    run_command_streams(&mut command, 300).unwrap()
}

#[test]
#[ignore = "requires the wasm32-unknown-unknown Rust target"]
fn generated_adapter_crate_passes_wasm32_clippy_and_rejects_bare_core_names() {
    let root = tempfile::tempdir().unwrap();
    let source = generate(root.path(), &configuration("Bridge"));
    write_native_fixture(root.path(), &source);
    let broken = source.replace(".map(BridgeReply::from)", ".map(Reply::from)");
    assert_ne!(broken, source);
    std::fs::write(root.path().join("binding/src/lib.rs"), broken).unwrap();
    let negative = lint(root.path());
    assert!(!negative.success);
    assert!(
        negative.stderr.contains("error[E0433]")
            && (negative.stderr.contains("cannot find type `Reply`")
                || negative.stderr.contains("use of undeclared type `Reply`")),
        "{}",
        negative.stderr
    );
    std::fs::write(root.path().join("binding/src/lib.rs"), source).unwrap();
    let repaired = lint(root.path());
    assert!(repaired.success, "{}\n{}", repaired.stdout, repaired.stderr);
}

#[test]
fn external_jsvalue_overrides_keep_the_serialization_fallback() {
    let mut config = configuration("Bridge");
    config.adapters.truncate(1);
    config.adapters[0].returns = Some("external::ForeignReply".into());
    config
        .wasm
        .as_mut()
        .unwrap()
        .type_overrides
        .insert("ForeignReply".into(), "JsValue".into());
    let bodies = alef::adapters::build_adapter_bodies(&config, alef::core::config::Language::Wasm).unwrap();
    let body = &bodies["Client.reply"];
    assert!(body.contains("serde_wasm_bindgen::to_value"), "{body}");
    assert!(!body.contains("BridgeForeignReply"), "{body}");
}

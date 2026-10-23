use super::{callable_test_support, generate_body};
use crate::core::config::{AdapterConfig, AdapterParam, AdapterPattern, Language, ResolvedCrateConfig};

pub(crate) fn adapter(pattern: AdapterPattern, core_path: &str) -> AdapterConfig {
    AdapterConfig {
        name: "public_events".into(),
        pattern,
        core_path: core_path.into(),
        owner_type: Some("Engine".into()),
        params: vec![AdapterParam {
            name: "req".into(),
            ty: "u32".into(),
            optional: false,
        }],
        request_type: Some("u32".into()),
        item_type: Some("Item".into()),
        error_type: Some("String".into()),
        returns: Some("u32".into()),
        gil_release: false,
        trait_name: None,
        trait_method: None,
        detect_async: false,
        skip_languages: vec![],
    }
}

#[test]
fn async_methods_compile_actual_core_calls_across_rust_producers() {
    let languages = [
        Language::Python,
        Language::Node,
        Language::Ruby,
        Language::Php,
        Language::Elixir,
        Language::Wasm,
        Language::Ffi,
        Language::R,
    ];
    let paths = [
        ("events", 6),
        ("Engine::events", 6),
        ("<Engine as Events>::events", 7),
        ("helpers::events", 8),
        ("generic::<u8>", 9),
        ("r#type", 10),
    ];
    let config = ResolvedCrateConfig {
        name: "sample".into(),
        ..Default::default()
    };
    let mut calls = Vec::new();
    for language in languages {
        for (path, expected) in paths {
            let body = generate_body(&adapter(AdapterPattern::AsyncMethod, path), language, &config).unwrap();
            assert!(
                !body.contains("public_events("),
                "public alias must not select a core callable"
            );
            let source = format!("async fn probe() {{ {body} }}");
            calls.push((callable_test_support::awaited_core_call(&source), expected));
        }
        let error = generate_body(&adapter(AdapterPattern::AsyncMethod, "events()"), language, &config).unwrap_err();
        assert!(error.to_string().contains("parsing adapter Rust callable"));
    }
    assert_eq!(calls.len(), 48);
    callable_test_support::verify_dispatch(&calls, false);
}

#[test]
fn empty_method_arguments_and_sync_function_paths_remain_distinct() {
    let config = ResolvedCrateConfig {
        name: "sample".into(),
        ..Default::default()
    };
    for language in [
        Language::Python,
        Language::Node,
        Language::Ruby,
        Language::Php,
        Language::Elixir,
        Language::Wasm,
        Language::Ffi,
        Language::R,
    ] {
        let mut adapter = adapter(AdapterPattern::AsyncMethod, "Engine::events");
        adapter.params.clear();
        let body = generate_body(&adapter, language, &config).unwrap();
        let call = callable_test_support::awaited_core_call(&format!("async fn probe() {{ {body} }}"));
        assert!(!call.contains(","), "no empty trailing argument: {call}");
        assert!(
            call.starts_with("Engine :: events (&"),
            "qualified dispatch must be retained: {call}"
        );
    }
    let mut adapter = adapter(AdapterPattern::SyncFunction, "helpers::events");
    adapter.returns = Some("JsValue".into());
    let body = crate::adapters::sync_function::generate_body(&adapter, Language::Wasm, &config).unwrap();
    assert!(body.starts_with("helpers::events(req.into())"), "{body}");
    assert!(!body.contains("self.inner") && !body.contains(".await"), "{body}");
}

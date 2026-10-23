use alef::adapters::streaming::generate_body;
use alef::core::config::{AdapterConfig, AdapterParam, AdapterPattern, Language, ResolvedCrateConfig};

fn adapter() -> AdapterConfig {
    AdapterConfig {
        name: "public_events".into(),
        pattern: AdapterPattern::Streaming,
        core_path: "sample::Engine::events".into(),
        owner_type: Some("Engine".into()),
        params: vec![AdapterParam {
            name: "req".into(),
            ty: "Request".into(),
            optional: false,
        }],
        request_type: Some("sample::Request".into()),
        item_type: Some("Item".into()),
        error_type: Some("String".into()),
        returns: None,
        gil_release: false,
        trait_name: None,
        trait_method: None,
        detect_async: false,
        skip_languages: vec![],
    }
}

#[test]
fn shared_streaming_producers_use_core_paths_and_preserve_bare_method_calls() {
    let config = ResolvedCrateConfig {
        name: "sample".into(),
        ..Default::default()
    };
    let cases = [
        (Language::Python, "inner", "core_req"),
        (Language::Node, "inner", "core_req"),
        (Language::Ruby, "self.inner", "core_req"),
        (Language::Php, "self.inner", "core_req"),
        (Language::Elixir, "inner", "core_req"),
        (Language::Wasm, "self.inner", "core_req"),
        (Language::Ffi, "client_ref", "request"),
        (Language::R, "self.inner", "core_req"),
        (Language::Dart, "inner", "core_req"),
    ];
    assert_eq!(cases.len(), 9);
    let mut checked = 0;
    for (language, receiver, args) in cases {
        for qualified in [false, true] {
            let mut adapter = adapter();
            if !qualified {
                adapter.core_path = "events".into();
            }
            let (body, definition) = generate_body(&adapter, language, &config).unwrap();
            let expected = if qualified {
                let borrowed = if language == Language::Ffi {
                    "client_ref".into()
                } else {
                    format!("&{receiver}")
                };
                format!("sample::Engine::events({borrowed}, {args})")
            } else {
                format!("{receiver}.events({args})")
            };
            let native = format!("async fn probe() {{ {body} }}\n{}", definition.unwrap_or_default());
            assert!(native.contains(&expected), "{language}: {native}");
            syn::parse_file(&native).unwrap_or_else(|error| panic!("{language}: {error}: {native}"));
            checked += 1;
        }
        let mut adapter = adapter();
        adapter.core_path = "events()".into();
        let error = generate_body(&adapter, language, &config).unwrap_err();
        assert!(error.to_string().contains("parsing adapter Rust callable"), "{error:#}");
    }
    assert_eq!(checked, 18);
}

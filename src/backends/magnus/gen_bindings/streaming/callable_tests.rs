use super::{StreamingAdapter, gen_streaming_method_body};
use crate::adapters::async_method::{callable_test_support, callable_tests::adapter};
use crate::core::config::AdapterPattern;

#[test]
fn streaming_methods_compile_the_configured_core_dispatch() {
    let paths = [
        ("events", 6),
        ("Engine::events", 6),
        ("<Engine as Events>::events", 7),
        ("helpers::events", 8),
        ("generic::<u8>", 9),
        ("r#type", 10),
    ];
    let mut calls = Vec::new();
    for (path, expected) in paths {
        let config = adapter(AdapterPattern::Streaming, path);
        let adapter = StreamingAdapter::from_config(&config, "Sample", "sample").unwrap();
        let body = gen_streaming_method_body(&adapter).unwrap();
        assert!(
            body.contains("fn public_events("),
            "public alias must remain on the binding"
        );
        calls.push((callable_test_support::awaited_core_call(&body), expected));
    }
    assert_eq!(calls.len(), 6);
    callable_test_support::verify_dispatch(&calls, false);
    let config = adapter(AdapterPattern::Streaming, "events()");
    let adapter = StreamingAdapter::from_config(&config, "Sample", "sample").unwrap();
    assert!(
        gen_streaming_method_body(&adapter)
            .unwrap_err()
            .to_string()
            .contains("parsing adapter Rust callable")
    );
}

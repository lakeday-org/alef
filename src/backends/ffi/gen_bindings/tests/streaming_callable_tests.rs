use super::super::helpers::gen_stream_handle_functions;
use crate::adapters::async_method::callable_test_support;

#[test]
fn stream_handles_compile_the_configured_core_dispatch_with_one_borrow() {
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
        let body =
            gen_stream_handle_functions("sample", "Engine", "public_events", path, "Item", "u32", "sample").unwrap();
        let call = callable_test_support::awaited_core_call(&body);
        assert!(
            !call.contains("& client_ref"),
            "borrowed receiver must not acquire another reference"
        );
        calls.push((call, expected));
    }
    assert_eq!(calls.len(), 6);
    callable_test_support::verify_dispatch(&calls, false);
    let error = gen_stream_handle_functions("sample", "Engine", "public_events", "events()", "Item", "u32", "sample")
        .unwrap_err();
    assert!(error.to_string().contains("parsing adapter Rust callable"));
}

use super::*;

/// Languages whose async-method adapter constructs its own tokio runtime. PHP is absent on
/// purpose: its async body drives the shared `WORKER_RUNTIME` static instead of building one,
/// so the stack size is set where that static is emitted. ~keep
const ASYNC_METHOD_RUNTIME_LANGUAGES: &[Language] = &[Language::Ruby, Language::Elixir, Language::R];

/// Streaming languages paired with the emitted artifact that actually carries their runtime.
///
/// The suffix selects an entry in the adapter-body map: `""` is the streaming method body,
/// `".__stream_struct__"` is the adapter's struct slot. Elixir is the one that differs — it
/// exposes the stream as a pair of standalone NIFs (`*_start` / `*_next`) emitted into the
/// struct slot, and leaves the method body a stub that says so. The runtime that drives the
/// core future lives in `*_start`, so that is the artifact to assert on; asserting on Elixir's
/// method body would be asserting against a stub that legitimately has no runtime at all.
/// `elixir_streaming_method_body_is_a_standalone_nif_stub_with_no_runtime` pins that split so
/// a future change which moves the runtime back into the method body cannot slip past this
/// table unnoticed. ~keep
const STREAMING_RUNTIME_ARTIFACTS: &[(Language, &str)] = &[
    (Language::Ruby, ""),
    (Language::R, ""),
    (Language::Dart, ""),
    (Language::Elixir, ".__stream_struct__"),
];

fn assert_runtime_has_explicit_worker_stack(body: &str, context: &str) {
    assert!(
        body.contains("tokio::runtime::Builder::new_multi_thread()"),
        "{context}: runtime must be built with the multi-thread builder. Got: {body}"
    );
    assert!(
        body.contains(".thread_stack_size("),
        "{context}: runtime must set an explicit worker stack size, or a deep consumer future \
         overflows tokio's ~2 MB default and aborts the process with SIGBUS. Got: {body}"
    );
    assert!(
        body.contains("16 * 1024 * 1024"),
        "{context}: worker stack size must be the project-wide 16 MiB value. Got: {body}"
    );
    assert!(
        !body.contains("tokio::runtime::Runtime::new()"),
        "{context}: `Runtime::new()` uses tokio's default (undersized) worker stack. Got: {body}"
    );
}

fn async_method_adapter() -> AdapterConfig {
    AdapterConfig {
        name: "process_async".to_string(),
        pattern: AdapterPattern::AsyncMethod,
        core_path: "process_async".to_string(),
        params: vec![AdapterParam {
            name: "request".to_string(),
            ty: "Request".to_string(),
            optional: false,
        }],
        returns: Some("Response".to_string()),
        error_type: Some("ProcessError".to_string()),
        owner_type: Some("MyClient".to_string()),
        item_type: None,
        gil_release: false,
        trait_name: None,
        trait_method: None,
        detect_async: false,
        request_type: None,

        skip_languages: vec![],
    }
}

pub(super) fn streaming_adapter() -> AdapterConfig {
    AdapterConfig {
        name: "stream_data".to_string(),
        pattern: AdapterPattern::Streaming,
        core_path: "stream_data".to_string(),
        params: vec![AdapterParam {
            name: "limit".to_string(),
            ty: "u32".to_string(),
            optional: false,
        }],
        returns: None,
        error_type: Some("StreamError".to_string()),
        owner_type: Some("DataClient".to_string()),
        item_type: Some("DataItem".to_string()),
        gil_release: false,
        trait_name: None,
        trait_method: None,
        detect_async: false,
        request_type: None,

        skip_languages: vec![],
    }
}

#[test]
fn async_method_adapters_build_runtimes_with_an_explicit_worker_stack() {
    for &language in ASYNC_METHOD_RUNTIME_LANGUAGES {
        let mut config = make_config(vec![language]);
        config.adapters = vec![async_method_adapter()];

        let bodies = build_adapter_bodies(&config, language).expect("build failed");
        let body = bodies
            .get("MyClient.process_async")
            .unwrap_or_else(|| panic!("{language}: expected an async-method body. Keys: {:?}", bodies.keys()));

        assert_runtime_has_explicit_worker_stack(body, &format!("{language} async-method adapter"));
    }
}

#[test]
fn streaming_adapters_build_runtimes_with_an_explicit_worker_stack() {
    for &(language, artifact_suffix) in STREAMING_RUNTIME_ARTIFACTS {
        let mut config = make_config(vec![language]);
        config.adapters = vec![streaming_adapter()];

        let bodies = build_adapter_bodies(&config, language).expect("build failed");
        let key = format!("DataClient.stream_data{artifact_suffix}");
        let body = bodies
            .get(&key)
            .unwrap_or_else(|| panic!("{language}: expected `{key}`. Keys: {:?}", bodies.keys()));

        assert_runtime_has_explicit_worker_stack(body, &format!("{language} streaming ({key})"));
    }
}

/// Justifies Elixir's entry in `STREAMING_RUNTIME_ARTIFACTS` pointing at the struct slot rather
/// than the method body. The method body is a stub reporting the standalone-NIF split, so it
/// builds no runtime — and that is a fact about the generator, not a convenience for the test
/// above. If Elixir ever starts driving the stream from the method body, this fails and the
/// artifact table must gain a second entry, rather than a real runtime quietly escaping the
/// worker-stack assertion. ~keep
#[test]
fn elixir_streaming_method_body_is_a_standalone_nif_stub_with_no_runtime() {
    let mut config = make_config(vec![Language::Elixir]);
    config.adapters = vec![streaming_adapter()];

    let bodies = build_adapter_bodies(&config, Language::Elixir).expect("build failed");
    let method_body = &bodies["DataClient.stream_data"];

    assert!(
        method_body.contains("streaming method emitted as standalone NIFs"),
        "Elixir's streaming method body must remain the standalone-NIF stub. Got: {method_body}"
    );
    assert!(
        !method_body.contains("tokio::runtime::"),
        "the stub must build no runtime; if it starts to, STREAMING_RUNTIME_ARTIFACTS must assert \
         on Elixir's method body too. Got: {method_body}"
    );

    let struct_slot = &bodies["DataClient.stream_data.__stream_struct__"];
    assert!(
        struct_slot.contains("pub fn dataclient_stream_data_start"),
        "the start NIF — the site that actually drives the core future — must be emitted into \
         the struct slot. Got: {struct_slot}"
    );
    assert!(
        struct_slot.contains("block_on"),
        "the start NIF must be the site that blocks on the core future. Got: {struct_slot}"
    );
}

/// The PHP async-method adapter must keep driving the shared `WORKER_RUNTIME` rather than
/// building a per-call runtime. If it ever starts constructing one, that runtime would escape
/// the stack-size guarantee the shared static carries, so pin the delegation explicitly. ~keep
#[test]
fn php_async_method_adapter_delegates_to_the_shared_worker_runtime() {
    let mut config = make_config(vec![Language::Php]);
    config.adapters = vec![async_method_adapter()];

    let bodies = build_adapter_bodies(&config, Language::Php).expect("build failed");
    let body = &bodies["MyClient.process_async"];

    assert!(
        body.contains("WORKER_RUNTIME.block_on"),
        "PHP async body must drive the shared worker runtime. Got: {body}"
    );
    assert!(
        !body.contains("tokio::runtime::Runtime::new()"),
        "PHP async body must not build its own default-stack runtime. Got: {body}"
    );
}

#[test]
fn php_streaming_adapter_uses_the_shared_runtime_with_a_wide_worker_stack() {
    use alef::backends::php::PhpBackend;
    use alef::core::backend::Backend;
    use alef::core::ir::ApiSurface;

    let mut config = make_config(vec![Language::Php]);
    config.adapters = vec![streaming_adapter()];
    let bodies = build_adapter_bodies(&config, Language::Php).expect("build failed");
    let method = &bodies["DataClient.stream_data"];
    let handle = &bodies["DataClient.stream_data.__stream_struct__"];
    assert!(method.contains("WORKER_RUNTIME"), "{method}");
    assert!(handle.contains("WORKER_RUNTIME.block_on(stream.next())"), "{handle}");
    assert!(!method.contains("tokio::runtime::Builder"), "{method}");
    let files = PhpBackend.generate_bindings(&ApiSurface::default(), &config).unwrap();
    let native: Vec<_> = files.iter().filter(|file| file.path.ends_with("lib.rs")).collect();
    assert_eq!(native.len(), 1);
    assert_runtime_has_explicit_worker_stack(&native[0].content, "PHP shared worker runtime");
}

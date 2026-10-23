//! The FFI last-error slot also carries the typed error's variant name and its whitelisted
//! introspection methods, so a consumer that only sees the C ABI (Go) can rebuild a typed error.

use super::common::resolved_one;
use crate::backends::ffi::gen_bindings::last_error::gen_last_error;
use crate::core::ir::{ApiSurface, ErrorDef, ErrorVariant, MethodDef, PrimitiveType, TypeRef};

fn config() -> crate::core::config::ResolvedCrateConfig {
    resolved_one(
        r#"
[workspace]
languages = ["ffi"]

[[crates]]
name = "sample"
sources = ["src/lib.rs"]

[crates.ffi]
prefix = "smp"
"#,
    )
}

fn method(name: &str, return_type: TypeRef) -> MethodDef {
    MethodDef {
        name: name.to_string(),
        return_type,
        ..MethodDef::default()
    }
}

fn sample_api() -> ApiSurface {
    ApiSurface {
        errors: vec![ErrorDef {
            exception_base: None,
            name: "SampleError".to_string(),
            rust_path: "sample::SampleError".to_string(),
            original_rust_path: String::new(),
            variants: vec![
                ErrorVariant {
                    name: "Timeout".to_string(),
                    is_unit: true,
                    ..Default::default()
                },
                ErrorVariant {
                    name: "RateLimited".to_string(),
                    ..Default::default()
                },
            ],
            doc: String::new(),
            methods: vec![
                method("status_code", TypeRef::Primitive(PrimitiveType::U16)),
                method("is_transient", TypeRef::Primitive(PrimitiveType::Bool)),
                method("error_type", TypeRef::String),
                method("retry_after", TypeRef::Optional(Box::new(TypeRef::Duration))),
            ],
            binding_excluded: false,
            binding_exclusion_reason: None,
            version: Default::default(),
        }],
        ..ApiSurface::default()
    }
}

#[test]
fn accessors_are_emitted_per_whitelisted_method_and_for_the_variant() {
    let module = gen_last_error(&sample_api(), &config().ffi_prefix(), "sample");

    for signature in [
        "pub unsafe extern \"C\" fn smp_last_error_variant() -> *const c_char",
        "pub unsafe extern \"C\" fn smp_last_error_status_code() -> u16",
        "pub unsafe extern \"C\" fn smp_last_error_is_transient() -> bool",
        "pub unsafe extern \"C\" fn smp_last_error_error_type() -> *const c_char",
        "pub unsafe extern \"C\" fn smp_last_error_retry_after() -> i64",
    ] {
        assert!(module.contains(signature), "missing `{signature}` in:\n{module}");
    }
    assert!(
        module.contains("sample::SampleError::Timeout => \"Timeout\""),
        "{module}"
    );
    assert!(
        module.contains("sample::SampleError::RateLimited { .. } => \"RateLimited\""),
        "{module}"
    );
    assert!(
        module.contains(
            "LAST_ERROR_RETRY_AFTER.with_borrow_mut(|c| *c = error.retry_after().map_or(-1, alef_ffi_duration_millis));"
        ),
        "{module}"
    );
    assert!(module.contains("fn clear_last_error_fields()"), "{module}");
}

#[test]
fn no_error_methods_means_only_the_variant_accessor_is_added() {
    let mut api = sample_api();
    api.errors[0].methods.clear();

    let module = gen_last_error(&api, &config().ffi_prefix(), "sample");

    assert!(module.contains("fn smp_last_error_variant()"), "{module}");
    assert!(!module.contains("smp_last_error_status_code"), "{module}");
    assert!(!module.contains("alef_ffi_duration_millis"), "{module}");
}

fn capture_body(module: &str) -> &str {
    let start = module.find("fn alef_ffi_capture_error_fields(").expect("capture fn");
    let end = module[start..].find("\nfn set_last_error(").expect("next fn") + start;
    &module[start..end]
}

#[test]
fn single_error_type_capture_has_no_trailing_return() {
    let module = gen_last_error(&sample_api(), &config().ffi_prefix(), "sample");

    assert!(!capture_body(&module).contains("return;"), "{module}");
}

#[test]
fn only_non_final_error_types_return_early_from_capture() {
    let mut api = sample_api();
    let mut second = api.errors[0].clone();
    second.name = "OtherError".to_string();
    second.rust_path = "sample::OtherError".to_string();
    api.errors.push(second);

    let module = gen_last_error(&api, &config().ffi_prefix(), "sample");

    assert_eq!(capture_body(&module).matches("return;").count(), 1, "{module}");
}

#[test]
fn generated_module_captures_resets_and_exposes_the_typed_error_fields() {
    let module = gen_last_error(&sample_api(), &config().ffi_prefix(), "sample");

    let source = format!(
        r#"
use std::cell::RefCell;
use std::ffi::{{c_char, CStr, CString}};

mod sample {{
    pub enum SampleError {{
        Timeout,
        RateLimited {{ retry_after: Option<std::time::Duration> }},
    }}
    impl SampleError {{
        pub fn status_code(&self) -> u16 {{
            match self {{ Self::Timeout => 408, Self::RateLimited {{ .. }} => 429 }}
        }}
        pub fn is_transient(&self) -> bool {{ true }}
        pub fn error_type(&self) -> &'static str {{
            match self {{ Self::Timeout => "Timeout", Self::RateLimited {{ .. }} => "RateLimited" }}
        }}
        pub fn retry_after(&self) -> Option<std::time::Duration> {{
            match self {{ Self::RateLimited {{ retry_after }} => *retry_after, _ => None }}
        }}
    }}
}}

{module}

fn text(pointer: *const c_char) -> Option<String> {{
    (!pointer.is_null()).then(|| unsafe {{ CStr::from_ptr(pointer) }}.to_str().unwrap().to_owned())
}}

fn main() {{
    set_last_error_from(
        &sample::SampleError::RateLimited {{ retry_after: Some(std::time::Duration::from_millis(2500)) }},
        "rate limited: slow down",
    );
    unsafe {{
        assert_eq!(smp_last_error_code(), 2, "an uncoded variant maps to the unknown code");
        assert_eq!(text(smp_last_error_context()).as_deref(), Some("rate limited: slow down"));
        assert_eq!(text(smp_last_error_variant()).as_deref(), Some("RateLimited"));
        assert_eq!(smp_last_error_status_code(), 429);
        assert!(smp_last_error_is_transient());
        assert_eq!(text(smp_last_error_error_type()).as_deref(), Some("RateLimited"));
        assert_eq!(smp_last_error_retry_after(), 2500);
    }}

    set_last_error_from(&sample::SampleError::Timeout, "timed out");
    unsafe {{
        assert_eq!(text(smp_last_error_variant()).as_deref(), Some("Timeout"));
        assert_eq!(smp_last_error_status_code(), 408);
        assert_eq!(smp_last_error_retry_after(), -1, "an absent Option<Duration> is -1, not the previous error's value");
    }}

    set_last_error(7, "a plain failure with no typed error value");
    unsafe {{
        assert_eq!(smp_last_error_code(), 7);
        assert!(smp_last_error_variant().is_null());
        assert_eq!(smp_last_error_status_code(), 0);
        assert!(!smp_last_error_is_transient());
        assert!(smp_last_error_error_type().is_null());
        assert_eq!(smp_last_error_retry_after(), -1);
    }}

    set_last_error_from(
        &sample::SampleError::RateLimited {{ retry_after: Some(std::time::Duration::from_secs(1)) }},
        "again",
    );
    clear_last_error();
    unsafe {{
        assert_eq!(smp_last_error_code(), 0);
        assert!(smp_last_error_context().is_null());
        assert!(smp_last_error_variant().is_null());
        assert_eq!(smp_last_error_status_code(), 0);
        assert!(smp_last_error_error_type().is_null());
        assert_eq!(smp_last_error_retry_after(), -1);
    }}

    set_last_error_from(&String::from("not a core error"), "string error");
    unsafe {{
        assert_eq!(smp_last_error_code(), 2);
        assert!(smp_last_error_variant().is_null());
    }}
}}
"#
    );

    let directory = tempfile::tempdir().expect("temporary directory");
    let source_path = directory.path().join("last_error_fields.rs");
    let binary_path = directory.path().join("last-error-fields-test");
    std::fs::write(&source_path, &source).expect("write compile harness");
    let compile = std::process::Command::new("rustc")
        .current_dir(directory.path())
        .args(["--edition=2024", "-o"])
        .arg(&binary_path)
        .arg(&source_path)
        .output()
        .expect("run rustc");
    assert!(
        compile.status.success(),
        "{}\n---source---\n{source}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = std::process::Command::new(&binary_path)
        .current_dir(directory.path())
        .output()
        .expect("run compiled harness");
    assert!(
        run.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}

fn generated_stream_next_transition() -> (String, String) {
    use quote::ToTokens;
    use syn::visit::Visit;

    #[derive(Default)]
    struct NextTransition(Vec<syn::ExprMatch>);
    impl<'ast> Visit<'ast> for NextTransition {
        fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
            if matches!(node.expr.as_ref(), syn::Expr::Path(path) if path.path.is_ident("next")) {
                self.0.push(node.clone());
            }
            syn::visit::visit_expr_match(self, node);
        }
    }
    let generated = crate::backends::ffi::gen_bindings::helpers::gen_stream_handle_functions(
        "smp",
        "Client",
        "watch",
        "watch",
        "Chunk",
        "sample::Request",
        "sample",
    )
    .unwrap();
    let parsed = syn::parse_file(&generated).expect("parse generated stream implementation");
    let carrier_name = "SmpClientWatchStreamHandleError";
    let carrier: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| match item {
            syn::Item::Struct(item) => item.ident == carrier_name,
            syn::Item::Impl(item) => {
                matches!(item.self_ty.as_ref(), syn::Type::Path(path) if path.path.is_ident(carrier_name))
            }
            _ => false,
        })
        .map(ToTokens::to_token_stream)
        .collect();
    assert_eq!(
        carrier.len(),
        2,
        "exercise the actual generated carrier declaration and implementation"
    );
    let mut visitor = NextTransition::default();
    visitor.visit_file(&parsed);
    assert_eq!(
        visitor.0.len(),
        1,
        "the real generated NEXT transition must be exercised"
    );
    (
        visitor.0.remove(0).into_token_stream().to_string(),
        quote::quote!(#(#carrier)*).to_string(),
    )
}

#[test]
fn deferred_stream_error_retains_native_identity_after_a_successful_chunk() {
    let mut api = sample_api();
    api.errors[0].variants[0].error_code = Some(108);
    api.errors[0].variants[1].error_code = Some(101);
    let module = gen_last_error(&api, &config().ffi_prefix(), "sample");
    let (transition, carrier) = generated_stream_next_transition();
    let source = format!(
        r#"
use std::cell::RefCell;
use std::ffi::{{c_char, CStr, CString}};

mod sample {{
    #[derive(Debug)]
    pub enum SampleError {{
        Timeout,
        RateLimited {{ retry_after: Option<std::time::Duration> }},
    }}
    impl std::fmt::Display for SampleError {{
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
            f.write_str("provider stream rejected the next chunk")
        }}
    }}
    impl std::error::Error for SampleError {{}}
    impl SampleError {{
        pub fn status_code(&self) -> u16 {{
            match self {{ Self::Timeout => 408, Self::RateLimited {{ .. }} => 429 }}
        }}
        pub fn is_transient(&self) -> bool {{ true }}
        pub fn error_type(&self) -> &'static str {{
            match self {{ Self::Timeout => "Timeout", Self::RateLimited {{ .. }} => "RateLimited" }}
        }}
        pub fn retry_after(&self) -> Option<std::time::Duration> {{
            match self {{ Self::RateLimited {{ retry_after }} => *retry_after, _ => None }}
        }}
    }}
}}

{module}
{carrier}

fn insert_handle(chunk: u64) -> Result<u64, String> {{ Ok(chunk) }}
fn set_handle_error(error: &String) {{ panic!("unexpected handle error: {{error}}") }}

fn advance(next: Option<Result<u64, SmpClientWatchStreamHandleError>>) -> u64 {{
    clear_last_error();
    let mut state = Some(());
    let guard = &mut state;
    {transition}
}}

fn text(pointer: *const c_char) -> Option<String> {{
    (!pointer.is_null()).then(|| unsafe {{ CStr::from_ptr(pointer) }}.to_str().unwrap().to_owned())
}}

fn main() {{
    if std::env::args().any(|arg| arg == "--unknown-error") {{
        set_last_error_from(
            &sample::SampleError::RateLimited {{ retry_after: Some(std::time::Duration::from_millis(2500)) }},
            "seed typed metadata",
        );
        let unknown = SmpClientWatchStreamHandleError::new(std::io::Error::other("opaque provider failure"));
        assert_eq!(advance(Some(Err(unknown))), 0);
        unsafe {{
            assert_eq!(smp_last_error_code(), 99);
            assert!(smp_last_error_variant().is_null());
            assert!(smp_last_error_error_type().is_null());
            assert_eq!(smp_last_error_status_code(), 0);
            assert_eq!(smp_last_error_retry_after(), -1);
            assert!(!smp_last_error_is_transient());
            assert!(text(smp_last_error_context()).unwrap().contains("opaque provider failure"));
        }}
        return;
    }}
    let error = SmpClientWatchStreamHandleError::new(sample::SampleError::RateLimited {{
        retry_after: Some(std::time::Duration::from_millis(2500)),
    }});
    let mut stream = [Ok(7), Err(error)].into_iter();
    assert_eq!(advance(stream.next()), 7, "a real successful item precedes the deferred failure");
    unsafe {{ assert_eq!(smp_last_error_code(), 0); }}
    assert_eq!(advance(stream.next()), 0);
    let expected_code = if std::env::args().any(|arg| arg == "--wrong-code") {{ 104 }} else {{ 101 }};
    unsafe {{
        assert_eq!(smp_last_error_code(), expected_code, "deferred error code must retain its core taxonomy");
        assert_eq!(text(smp_last_error_variant()).as_deref(), Some("RateLimited"));
        assert_eq!(smp_last_error_status_code(), 429);
        assert_eq!(smp_last_error_retry_after(), 2500);
        assert!(smp_last_error_is_transient());
        assert_eq!(text(smp_last_error_error_type()).as_deref(), Some("RateLimited"));
        assert!(text(smp_last_error_context()).unwrap().contains("provider stream rejected the next chunk"));
    }}
    assert_eq!(advance(stream.next()), 0);
    unsafe {{
        assert_eq!(smp_last_error_code(), 0, "clean EOF must not retain the preceding error");
        assert!(smp_last_error_variant().is_null());
        assert!(smp_last_error_context().is_null());
        assert_eq!(smp_last_error_status_code(), 0);
        assert_eq!(smp_last_error_retry_after(), -1);
        assert!(!smp_last_error_is_transient());
    }}
}}
"#
    );
    let directory = tempfile::tempdir().expect("temporary directory");
    let source_path = directory.path().join("deferred_stream.rs");
    let binary_path = directory.path().join("deferred-stream-test");
    std::fs::write(&source_path, &source).expect("write generated transition harness");
    let compile = std::process::Command::new("rustc")
        .current_dir(directory.path())
        .args(["--edition=2024", "-o"])
        .arg(&binary_path)
        .arg(&source_path)
        .output()
        .expect("run rustc");
    assert!(
        compile.status.success(),
        "{}\n{source}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let unknown = std::process::Command::new(&binary_path)
        .arg("--unknown-error")
        .output()
        .expect("run unknown error control");
    assert!(unknown.status.success(), "{}", String::from_utf8_lossy(&unknown.stderr));
    let run = std::process::Command::new(&binary_path)
        .output()
        .expect("run NEXT transition");
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let negative = std::process::Command::new(&binary_path)
        .arg("--wrong-code")
        .output()
        .expect("run wrong taxonomy control");
    assert!(
        !negative.status.success(),
        "a deliberately wrong expected taxonomy must fail"
    );
}

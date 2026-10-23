mod dart;
mod php;
mod type_paths;

pub(crate) use dart::{generate_body as dart_body, transport_names as dart_transport_names};

use heck::ToSnakeCase;
use php::gen_php_body;
use type_paths::source_type;

use crate::core::config::{AdapterConfig, Language, ResolvedCrateConfig};

/// Generate the method body and optionally a struct definition for a streaming adapter.
///
/// Returns `(method_body, Option<struct_definition>)`.
/// The struct definition is only produced for languages that need a separate iterator struct
/// (currently Python/PyO3).
pub fn generate_body(
    adapter: &AdapterConfig,
    language: Language,
    config: &ResolvedCrateConfig,
) -> anyhow::Result<(String, Option<String>)> {
    let lang_str = language.to_string();
    if adapter.skip_languages.iter().any(|l| l == &lang_str) {
        return Ok((String::new(), None));
    }
    match language {
        Language::Python => gen_python_body(adapter, config),
        Language::Node => gen_node_body(adapter, config),
        Language::Ruby => gen_ruby_body(adapter, config),
        Language::Php => gen_php_body(adapter, config),
        Language::Elixir => gen_elixir_body(adapter, config),
        Language::Wasm => gen_wasm_body(adapter, config),
        Language::Ffi => gen_ffi_body(adapter, config),
        Language::Go => Ok(gen_go_body(adapter)),
        Language::Java => Ok(gen_java_body(adapter)),
        Language::Csharp => Ok(gen_csharp_body(adapter)),
        Language::R => gen_r_body(adapter, config),
        Language::Rust | Language::C | Language::Jni => {
            anyhow::bail!("Rust/C/JNI do not need generated binding adapters")
        }
        Language::Dart => dart_body(adapter, config, false),
        Language::Kotlin | Language::KotlinAndroid | Language::Swift | Language::Gleam | Language::Zig => {
            anyhow::bail!("Phase 1: {language} backend not yet implemented")
        }
    }
}

/// Build the call arguments referencing `_core` locals created by the method codegen.
///
/// The regular method codegen already emits `let {name}_core: CoreType = {name}.into();`
/// for each parameter, so the adapter body must use those converted locals — not call
/// `.into()` a second time (which would trigger a use-after-move error).
fn call_args(adapter: &AdapterConfig) -> Vec<String> {
    adapter.params.iter().map(|p| format!("core_{}", p.name)).collect()
}

/// Build conversion let-bindings for core types.
fn core_let_bindings(adapter: &AdapterConfig, core_import: &str) -> Vec<String> {
    adapter
        .params
        .iter()
        .map(|p| {
            let ty = source_type(&p.ty, core_import);
            if p.optional {
                format!(
                    "let core_{name} = {name}.map(|v| -> {ty} {{ v.into() }});",
                    name = p.name,
                    ty = ty,
                )
            } else {
                format!("let core_{name}: {ty} = {name}.into();", name = p.name, ty = ty,)
            }
        })
        .collect()
}

/// Build conversion let-bindings for core types, cloning the input first.
/// Used by PHP which passes struct params by reference.
fn core_let_bindings_cloned(adapter: &AdapterConfig, core_import: &str) -> Vec<String> {
    adapter
        .params
        .iter()
        .map(|p| {
            let ty = source_type(&p.ty, core_import);
            if p.optional {
                format!(
                    "let core_{name} = {name}.as_ref().map(|v| -> {ty} {{ v.clone().into() }});",
                    name = p.name,
                    ty = ty,
                )
            } else {
                format!("let core_{name}: {ty} = {name}.clone().into();", name = p.name, ty = ty,)
            }
        })
        .collect()
}

/// Get the iterator struct name from the adapter name.
fn iterator_name(adapter: &AdapterConfig) -> String {
    to_pascal_case(&adapter.name) + "Iterator"
}

fn gen_python_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> anyhow::Result<(String, Option<String>)> {
    let item_type = adapter.item_type.as_deref().unwrap_or("()");
    let error_type = adapter.error_type.as_deref().unwrap_or("anyhow::Error");
    let core_import = config.core_import_name();
    let iter_name = iterator_name(adapter);

    let args = call_args(adapter);
    let core_call = super::rust_callable::receiver_call(&adapter.core_path, "inner", "&inner", &args.join(", "))?;

    let anext_err_handler = if error_type != "anyhow::Error" {
        let simple_name = error_type.split("::").last().unwrap_or(error_type);
        let fn_name = format!("{}_to_py_err", simple_name.to_snake_case());
        format!("Err({fn_name}(e))")
    } else {
        "Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))".to_string()
    };

    let struct_def = format!(
        "#[pyclass]\n\
         pub struct {iter_name} {{\n    \
             receiver: Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Result<{core_import}::{item_type}, {core_import}::{error_type}>>>>,\n\
         }}\n\
         \n\
         #[pymethods]\n\
         impl {iter_name} {{\n    \
             fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {{ slf }}\n\
             \n    \
             fn __anext__<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {{\n        \
                 let receiver = self.receiver.clone();\n        \
                 pyo3_async_runtimes::tokio::future_into_py(py, async move {{\n            \
                     let mut rx = receiver.lock().await;\n            \
                     match rx.recv().await {{\n                \
                         Some(Ok(chunk)) => Ok({item_type}::from(chunk)),\n                \
                         Some(Err(e)) => {anext_err_handler},\n                \
                         None => Err(pyo3::exceptions::PyStopAsyncIteration::new_err(())),\n            \
                     }}\n        \
                 }}).map(Some)\n    \
             }}\n\
         }}"
    );

    let let_bindings = core_let_bindings(adapter, &core_import);
    let bindings_block = if let_bindings.is_empty() {
        String::new()
    } else {
        format!("{}\n    ", let_bindings.join("\n    "))
    };

    // A closed `PyAny` async iterator (e.g. `aclose()`) drops all receivers, so `tx.closed()`
    // resolves and we stop pulling from `stream` immediately instead of waiting for the next
    // item. `futures::StreamExt::next` is cancel-safe here: each poll either returns `Pending`
    // (nothing produced, safe to drop) or `Ready` (the item is already claimed by that poll, so
    // `select!` never discards a produced item). Dropping `stream` on the way out stops any
    // further upstream work at once rather than waiting for it to finish on its own.

    let method_body = crate::codegen::template_env::render(
        "adapters/streaming_python_body.rs.jinja",
        crate::alef_context! {
            bindings_block => &bindings_block,
            core_call => &core_call,
            iter_name => &iter_name,
        },
    );

    Ok((method_body, Some(struct_def)))
}

fn gen_node_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> anyhow::Result<(String, Option<String>)> {
    let prefix = config.node_type_prefix();
    let raw_item = adapter.item_type.as_deref().unwrap_or("()");
    let item_type = if raw_item == "()" {
        raw_item.to_string()
    } else {
        format!("{prefix}{raw_item}")
    };
    let core_import = config.core_import_name();
    let iter_name = iterator_name(adapter);

    let args = call_args(adapter);
    let core_call = super::rust_callable::receiver_call(&adapter.core_path, "inner", "&inner", &args.join(", "))?;

    let let_bindings = core_let_bindings(adapter, &core_import);
    let bindings_block = if let_bindings.is_empty() {
        String::new()
    } else {
        format!("{}\n    ", let_bindings.join("\n    "))
    };

    let struct_def = format!(
        "#[napi(async_iterator)]\n\
         pub struct {iter_name} {{\n    \
             receiver: Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<napi::Result<{item_type}>>>>,\n\
         }}\n\
         \n\
         impl napi::bindgen_prelude::AsyncGenerator for {iter_name} {{\n    \
             type Yield = {item_type};\n    \
             type Next = napi::bindgen_prelude::Unknown<'static>;\n    \
             type Return = napi::bindgen_prelude::Unknown<'static>;\n\
             \n    \
             fn next(\n        \
                 &mut self,\n        \
                 _value: Option<napi::bindgen_prelude::Unknown<'static>>,\n    \
             ) -> impl std::future::Future<Output = napi::Result<Option<{item_type}>>> + Send + 'static {{\n        \
                 let receiver = self.receiver.clone();\n        \
                 async move {{\n            \
                     let mut rx = receiver.lock().await;\n            \
                     match rx.recv().await {{\n                \
                         Some(res) => res.map(Some),\n                \
                         None => Ok(None),\n            \
                     }}\n        \
                 }}\n    \
             }}\n\
         }}"
    );

    // See the pyo3 body above for why `tx.closed()` is raced against `stream.next()` and why
    // that race cannot drop a produced item.

    // The stream is opened before the forwarding task starts, so a failed open (401, 429)
    // rejects the call itself, as the Rust and Go bindings do. Only errors after the first
    // chunk travel through the channel and arrive on read.
    let method_body = crate::codegen::template_env::render(
        "adapters/streaming_node_body.rs.jinja",
        crate::alef_context! {
            bindings_block => &bindings_block,
            core_call => &core_call,
            item_type => &item_type,
            iter_name => &iter_name,
        },
    );

    Ok((method_body, Some(struct_def)))
}

fn gen_ruby_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> anyhow::Result<(String, Option<String>)> {
    let item_type = adapter.item_type.as_deref().unwrap_or("()");
    let core_import = config.core_import_name();

    let args = call_args(adapter);
    let core_call =
        super::rust_callable::receiver_call(&adapter.core_path, "self.inner", "&self.inner", &args.join(", "))?;

    let let_bindings = core_let_bindings(adapter, &core_import);
    let bindings_block = if let_bindings.is_empty() {
        String::new()
    } else {
        format!("{}\n    ", let_bindings.join("\n    "))
    };

    let body = crate::codegen::template_env::render(
        "adapters/streaming_ruby_body.rs.jinja",
        crate::alef_context! {
            bindings_block => &bindings_block,
            core_call => &core_call,
            item_type => &item_type,
        },
    );

    Ok((body, None))
}

/// The Rust name of the PHP class that holds the open stream of a streaming adapter.
pub(crate) fn php_stream_handle_name(adapter: &AdapterConfig) -> String {
    format!(
        "{}{}Handle",
        to_pascal_case(adapter.owner_type.as_deref().unwrap_or("")),
        to_pascal_case(&adapter.name)
    )
}

/// Streaming adapter for Rustler/Elixir.
///
/// Emits a pair of standalone `#[rustler::nif]` functions plus a per-adapter
/// handle resource. The pair is `{owner_lc}_{name}_start` (returns the resource)
/// and `{owner_lc}_{name}_next` (returns chunk JSON or `nil` on end-of-stream).
/// The Elixir wrapper drives the pair via `Stream.unfold/2`.
///
/// The corresponding method on the owner type is suppressed in `mod.rs` to avoid
/// double-emitting a NIF for the same method name.
fn gen_elixir_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> anyhow::Result<(String, Option<String>)> {
    let core_path = &adapter.core_path;
    let item_type = adapter.item_type.as_deref().unwrap_or("()");
    let error_type = adapter.error_type.as_deref().unwrap_or("anyhow::Error");
    let core_import = config.core_import_name();
    let owner_type = adapter.owner_type.as_deref().unwrap_or("");
    let owner_lc = owner_type.to_lowercase();
    let adapter_name = &adapter.name;
    let handle_struct = format!("{}{}Handle", to_pascal_case(owner_type), to_pascal_case(adapter_name));
    let start_fn = format!("{owner_lc}_{adapter_name}_start");
    let next_fn = format!("{owner_lc}_{adapter_name}_next");
    let req_param_name = adapter
        .params
        .first()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "req".to_string());
    let req_param_type = adapter
        .params
        .first()
        .map(|p| p.ty.clone())
        .unwrap_or_else(|| "rustler::Term".to_string());

    let args = call_args(adapter);
    let core_call = super::rust_callable::receiver_call(&adapter.core_path, "inner", "&inner", &args.join(", "))?;

    let let_bindings = core_let_bindings(adapter, &core_import);
    let bindings_block = if let_bindings.is_empty() {
        String::new()
    } else {
        format!("{}\n    ", let_bindings.join("\n    "))
    };

    let body = format!(
        "Err::<String, String>(\"streaming method emitted as standalone NIFs ({start_fn}/{next_fn})\".to_string())"
    );

    let struct_def = crate::codegen::template_env::render(
        "adapters/streaming_elixir_body.rs.jinja",
        crate::alef_context! {
            owner_type => &owner_type,
            core_path => &core_path,
            next_fn => &next_fn,
            handle_struct => &handle_struct,
            core_import => &core_import,
            item_type => &item_type,
            error_type => &error_type,
            start_fn => &start_fn,
            req_param_name => &req_param_name,
            req_param_type => &req_param_type,
            bindings_block => &bindings_block,
            core_call => &core_call,
        },
    );

    Ok((body, Some(struct_def)))
}

fn gen_wasm_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> anyhow::Result<(String, Option<String>)> {
    let prefix = config.wasm_type_prefix();
    let raw_item = adapter.item_type.as_deref().unwrap_or("JsValue");
    let item_type = if raw_item == "()" || raw_item == "JsValue" {
        raw_item.to_string()
    } else {
        format!("{prefix}{raw_item}")
    };
    let core_import = config.core_import_name();
    let iter_name = iterator_name(adapter);

    let args = call_args(adapter);
    let core_call =
        super::rust_callable::receiver_call(&adapter.core_path, "self.inner", "&self.inner", &args.join(", "))?;

    let let_bindings = core_let_bindings(adapter, &core_import);
    let bindings_block = if let_bindings.is_empty() {
        String::new()
    } else {
        format!("{}\n    ", let_bindings.join("\n    "))
    };

    let struct_def = format!(
        "#[wasm_bindgen]\n\
         pub struct {iter_name} {{\n    \
             // Receiver wrapped in RefCell for interior mutability (WASM is single-threaded)\n    \
             receiver: std::cell::RefCell<futures::channel::mpsc::Receiver<Result<{item_type}, JsValue>>>,\n\
         }}\n\
         \n\
         #[wasm_bindgen]\n\
         impl {iter_name} {{\n    \
             #[wasm_bindgen]\n    \
             pub async fn next(&self) -> Result<JsValue, JsValue> {{\n        \
                 use futures::stream::StreamExt;\n        \
                 let mut rx = self.receiver.borrow_mut();\n        \
                 match rx.next().await {{\n            \
                     Some(Ok(item)) => Ok(JsValue::from(item)),\n            \
                     Some(Err(e)) => Err(e),\n            \
                     None => Ok(JsValue::null()),\n        \
                 }}\n    \
             }}\n\
         }}"
    );

    // The core call is awaited here, in the `async` wasm-bindgen method, so a server that refuses
    // the stream rejects the call itself. Only the forward loop runs in the spawned task.
    let method_body = crate::codegen::template_env::render(
        "adapters/streaming_wasm_body.rs.jinja",
        crate::alef_context! {
            bindings_block => &bindings_block,
            core_call => &core_call,
            item_type => &item_type,
            iter_name => &iter_name,
        },
    );

    Ok((method_body, Some(struct_def)))
}

fn gen_ffi_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> anyhow::Result<(String, Option<String>)> {
    let core_call = super::rust_callable::receiver_call(&adapter.core_path, "client_ref", "client_ref", "request")?;
    let prefix = config.ffi_prefix();
    let request_type = adapter.request_type.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "streaming adapter '{}': `request_type` is required for FFI body generation \
             (e.g. request_type = \"my_crate::MyRequest\")",
            adapter.name,
        )
    })?;

    let body = crate::codegen::template_env::render(
        "adapters/streaming_ffi_body.rs.jinja",
        crate::alef_context! {
            prefix => &prefix,
            name => &adapter.name,
            request_type => &request_type,
            core_call => &core_call,
        },
    );

    Ok((body, None))
}

fn gen_go_body(adapter: &AdapterConfig) -> (String, Option<String>) {
    let body = format!("compile_error!(\"streaming not supported via FFI: {}\")", adapter.name);
    (body, None)
}

fn gen_java_body(adapter: &AdapterConfig) -> (String, Option<String>) {
    let body = format!("compile_error!(\"streaming not supported via FFI: {}\")", adapter.name);
    (body, None)
}

fn gen_csharp_body(adapter: &AdapterConfig) -> (String, Option<String>) {
    let body = format!("compile_error!(\"streaming not supported via FFI: {}\")", adapter.name);
    (body, None)
}

fn gen_r_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> anyhow::Result<(String, Option<String>)> {
    let item_type = adapter.item_type.as_deref().unwrap_or("Robj");
    let core_import = config.core_import_name();

    let args = call_args(adapter);
    let core_call =
        super::rust_callable::receiver_call(&adapter.core_path, "self.inner", "&self.inner", &args.join(", "))?;

    let let_bindings = core_let_bindings(adapter, &core_import);
    let bindings_block = if let_bindings.is_empty() {
        String::new()
    } else {
        format!("{}\n    ", let_bindings.join("\n    "))
    };

    let body = crate::codegen::template_env::render(
        "adapters/streaming_r_body.rs.jinja",
        crate::alef_context! {
            bindings_block => &bindings_block,
            core_call => &core_call,
            item_type => &item_type,
        },
    );

    Ok((body, None))
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect()
}

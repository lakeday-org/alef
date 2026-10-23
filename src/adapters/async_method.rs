use heck::{ToLowerCamelCase, ToSnakeCase};

use crate::core::config::{AdapterConfig, Language, ResolvedCrateConfig};

/// Generate just the method body (what goes inside `{ ... }`) for an async method adapter.
pub fn generate_body(
    adapter: &AdapterConfig,
    language: Language,
    config: &ResolvedCrateConfig,
) -> anyhow::Result<String> {
    let body = match language {
        Language::Python => gen_python_body(adapter, config)?,
        Language::Node => gen_node_body(adapter, config)?,
        Language::Ruby => gen_ruby_body(adapter, config)?,
        Language::Php => gen_php_body(adapter, config)?,
        Language::Elixir => gen_elixir_body(adapter, config)?,
        Language::Wasm => gen_wasm_body(adapter, config)?,
        Language::Ffi => gen_ffi_body(adapter, config)?,
        Language::Go => gen_go_body(adapter, config),
        Language::Java => gen_java_body(adapter, config),
        Language::Csharp => gen_csharp_body(adapter, config),
        Language::R => gen_r_body(adapter, config)?,
        Language::Rust | Language::C | Language::Jni => {
            anyhow::bail!("Rust/C/JNI do not need generated binding adapters")
        }
        Language::Kotlin
        | Language::KotlinAndroid
        | Language::Swift
        | Language::Dart
        | Language::Gleam
        | Language::Zig => {
            anyhow::bail!("Phase 1: {language} backend not yet implemented")
        }
    };
    Ok(body)
}

/// Returns true for primitive Rust types that don't live in any crate namespace.
fn is_builtin_type(ty: &str) -> bool {
    matches!(
        ty,
        "String"
            | "&str"
            | "bool"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "f32"
            | "f64"
            | "usize"
            | "isize"
    )
}

/// Build the call arguments with `.into()` conversion.
/// For `String`/`&str` params, passes `.as_str()` / `.as_deref()` so the
/// call site receives `&str` when the core method expects it.
pub(crate) fn call_args(adapter: &AdapterConfig) -> Vec<String> {
    adapter
        .params
        .iter()
        .map(|p| {
            if p.ty == "String" || p.ty == "&str" {
                if p.optional {
                    format!("{}.as_deref()", p.name)
                } else {
                    format!("{}.as_str()", p.name)
                }
            } else if p.optional {
                format!("{}.map(Into::into)", p.name)
            } else {
                format!("{}.into()", p.name)
            }
        })
        .collect()
}

/// Build the call arguments with `.clone().into()` for backends that pass by reference (PHP).
/// PHP signatures use lowerCamelCase parameter names (see `gen_php_function_params` in
/// the PHP backend), so the body must reference parameters by their camelCased identifiers.
fn call_args_cloned(adapter: &AdapterConfig) -> Vec<String> {
    adapter
        .params
        .iter()
        .map(|p| {
            let name = p.name.to_lower_camel_case();
            if p.ty == "String" || p.ty == "&str" {
                if p.optional {
                    format!("{name}.as_deref()")
                } else {
                    format!("{name}.as_str()")
                }
            } else if p.optional {
                format!("{name}.map(|v| v.clone().into())")
            } else {
                format!("{name}.clone().into()")
            }
        })
        .collect()
}

/// Build conversion let-bindings for core types (used in Python async).
fn core_let_bindings(adapter: &AdapterConfig, core_import: &str) -> Vec<String> {
    adapter
        .params
        .iter()
        .map(|p| {
            let ty_path = if is_builtin_type(&p.ty) {
                p.ty.clone()
            } else {
                format!("{core_import}::{}", p.ty)
            };
            if p.optional {
                format!(
                    "let core_{name} = {name}.map(|v| -> {ty_path} {{ v.into() }});",
                    name = p.name,
                )
            } else {
                format!("let core_{name}: {ty_path} = {name}.into();", name = p.name,)
            }
        })
        .collect()
}

/// Build core call arguments (prefixed with core_) for use in Python async bodies.
/// For `String`/`&str` params, passes `.as_str()` / `.as_deref()` so the
/// call site receives `&str` when the core method expects it.
fn core_call_args(adapter: &AdapterConfig) -> Vec<String> {
    adapter
        .params
        .iter()
        .map(|p| {
            if p.ty == "String" || p.ty == "&str" {
                if p.optional {
                    format!("core_{}.as_deref()", p.name)
                } else {
                    format!("core_{}.as_str()", p.name)
                }
            } else {
                format!("core_{}", p.name)
            }
        })
        .collect()
}

fn gen_python_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> anyhow::Result<String> {
    let raw_returns = adapter.returns.as_deref().unwrap_or("()");
    let core_import = config.core_import_name();

    let let_bindings = core_let_bindings(adapter, &core_import);
    let core_args = core_call_args(adapter);
    let core_call = super::rust_callable::receiver_call(&adapter.core_path, "inner", "&inner", &core_args.join(", "))?;

    let ok_expr = if raw_returns == "bytes::Bytes" {
        "Ok(result.to_vec())".to_string()
    } else {
        format!("Ok({raw_returns}::from(result))")
    };

    let err_mapper = if let Some(ref et) = adapter.error_type {
        let simple_name = et.split("::").last().unwrap_or(et);
        let fn_name = format!("{}_to_py_err", simple_name.to_snake_case());
        format!(".map_err({fn_name})")
    } else {
        ".map_err(|e| PyErr::new::<PyRuntimeError, _>(e.to_string()))".to_string()
    };

    Ok(crate::codegen::template_env::render(
        "adapters/async_python_body.rs.jinja",
        crate::alef_context! {
            let_bindings => let_bindings, core_call => core_call,
            err_mapper => err_mapper, ok_expr => ok_expr,
        },
    )
    .trim_end()
    .to_owned())
}

fn gen_node_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> anyhow::Result<String> {
    let prefix = config.node_type_prefix();
    let raw_returns = adapter.returns.as_deref().unwrap_or("()");

    let is_bytes = raw_returns == "bytes::Bytes";
    let map_expr = if raw_returns == "()" {
        format!(".map({raw_returns}::from)")
    } else if is_bytes {
        ".map(|b| napi::bindgen_prelude::Buffer::from(b.to_vec()))".to_string()
    } else {
        format!(".map({prefix}{raw_returns}::from)")
    };

    let args = call_args(adapter);

    if args.is_empty() {
        let core_call = super::rust_callable::receiver_call(&adapter.core_path, "self.inner", "&self.inner", "")?;
        Ok(crate::codegen::template_env::render(
            "adapters/async_node_empty.rs.jinja",
            crate::alef_context! { core_call => core_call, map_expr => map_expr },
        )
        .trim_end()
        .to_owned())
    } else {
        let core_call =
            super::rust_callable::receiver_call(&adapter.core_path, "self.inner", "&self.inner", "core_req")?;
        let call_str = args.join(", ");
        Ok(crate::codegen::template_env::render(
            "adapters/async_node_request.rs.jinja",
            crate::alef_context! { call_str => call_str, core_call => core_call, map_expr => map_expr },
        )
        .trim_end()
        .to_owned())
    }
}

fn gen_ruby_body(adapter: &AdapterConfig, _config: &ResolvedCrateConfig) -> anyhow::Result<String> {
    let raw_returns = adapter.returns.as_deref().unwrap_or("()");

    let map_expr = if raw_returns == "bytes::Bytes" {
        ".map(|b| b.to_vec())".to_string()
    } else {
        format!(".map({raw_returns}::from)")
    };

    let args = call_args(adapter);

    if args.is_empty() {
        let core_call = super::rust_callable::receiver_call(&adapter.core_path, "self.inner", "&self.inner", "")?;
        Ok(crate::codegen::template_env::render(
            "adapters/async_ruby_empty.rs.jinja",
            crate::alef_context! { core_call => core_call, map_expr => map_expr },
        )
        .trim_end()
        .to_owned())
    } else {
        let core_call =
            super::rust_callable::receiver_call(&adapter.core_path, "self.inner", "&self.inner", "core_req")?;
        let call_str = args.join(", ");
        Ok(crate::codegen::template_env::render(
            "adapters/async_ruby_request.rs.jinja",
            crate::alef_context! { call_str => call_str, core_call => core_call, map_expr => map_expr },
        )
        .trim_end()
        .to_owned())
    }
}

fn gen_php_body(adapter: &AdapterConfig, _config: &ResolvedCrateConfig) -> anyhow::Result<String> {
    let raw_returns = adapter.returns.as_deref().unwrap_or("()");

    let args = call_args_cloned(adapter);

    let inner_call =
        super::rust_callable::receiver_call(&adapter.core_path, "self.inner", "&self.inner", &args.join(", "))?;

    let map_expr = if raw_returns == "bytes::Bytes" {
        ".map(|b| b.to_vec())".to_string()
    } else {
        format!(".map({raw_returns}::from)")
    };

    Ok(crate::codegen::template_env::render(
        "adapters/async_php_body.rs.jinja",
        crate::alef_context! { inner_call => inner_call, map_expr => map_expr },
    )
    .trim_end()
    .to_owned())
}

fn gen_elixir_body(adapter: &AdapterConfig, _config: &ResolvedCrateConfig) -> anyhow::Result<String> {
    let raw_returns = adapter.returns.as_deref().unwrap_or("()");

    let map_expr = if raw_returns == "bytes::Bytes" {
        ".map(|b| b.to_vec())".to_string()
    } else {
        format!(".map({raw_returns}::from)")
    };

    let args = call_args(adapter);
    let core_call = super::rust_callable::receiver_call(
        &adapter.core_path,
        "resource.inner",
        "&resource.inner",
        &args.join(", "),
    )?;

    Ok(crate::codegen::template_env::render(
        "adapters/async_elixir_body.rs.jinja",
        crate::alef_context! { core_call => core_call, map_expr => map_expr },
    )
    .trim_end()
    .to_owned())
}

fn gen_wasm_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> anyhow::Result<String> {
    crate::backends::wasm::gen_bindings::adapters::generate_body(adapter, config)
}

fn gen_ffi_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> anyhow::Result<String> {
    let prefix = config.ffi_prefix();
    let owner_type = adapter.owner_type.as_deref().unwrap_or("Self");
    let owner_snake = to_snake_case(owner_type);
    let _ = prefix;
    let _ = owner_snake;

    let conversions: Vec<String> = adapter
        .params
        .iter()
        .map(|p| {
            if p.ty == "String" || p.ty == "&str" {
                crate::codegen::template_env::render(
                    "adapters/async_ffi_string_parameter.rs.jinja",
                    crate::alef_context! { name => p.name },
                )
                .trim_end()
                .to_owned()
            } else {
                crate::codegen::template_env::render(
                    "adapters/async_ffi_json_parameter.rs.jinja",
                    crate::alef_context! { name => p.name, ty => p.ty },
                )
                .trim_end()
                .to_owned()
            }
        })
        .collect();

    let call_args_list: Vec<String> = adapter.params.iter().map(|p| p.name.clone()).collect();
    let core_call = super::rust_callable::receiver_call(
        &adapter.core_path,
        "client.inner",
        "&client.inner",
        &call_args_list.join(", "),
    )?;

    Ok(crate::codegen::template_env::render(
        "adapters/async_ffi_body.rs.jinja",
        crate::alef_context! { conversions => conversions, core_call => core_call },
    )
    .trim_end()
    .to_owned())
}

fn gen_go_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> String {
    let name = &adapter.name;
    let prefix = config.ffi_prefix();
    let returns = adapter.returns.as_deref().unwrap_or("string");
    let owner_type = adapter.owner_type.as_deref().unwrap_or("Client");
    let owner_snake = to_snake_case(owner_type);

    let marshal_block: Vec<String> = adapter
        .params
        .iter()
        .filter(|p| p.ty != "String" && p.ty != "&str")
        .map(|p| {
            format!(
                "{name}JSON, err := json.Marshal({name})\n    \
                 if err != nil {{\n        \
                     return nil, err\n    \
                 }}",
                name = p.name,
            )
        })
        .collect();

    let c_call_args: Vec<String> = adapter
        .params
        .iter()
        .map(|p| {
            if p.ty == "String" || p.ty == "&str" {
                format!("C.CString({})", p.name)
            } else {
                format!("C.CString(string({name}JSON))", name = p.name)
            }
        })
        .collect();

    let call_str = c_call_args.join(", ");
    let marshal_str = if marshal_block.is_empty() {
        String::new()
    } else {
        format!("{}\n    ", marshal_block.join("\n    "))
    };

    format!(
        "{marshal_str}\
         result := C.{prefix}_{owner_snake}_{name}(c.ptr, {call_str})\n    \
         if result == nil {{\n        \
             return nil, fmt.Errorf(\"%s\", lastError())\n    \
         }}\n    \
         defer C.free(unsafe.Pointer(result))\n    \
         var out {returns}\n    \
         if err := json.Unmarshal([]byte(C.GoString(result)), &out); err != nil {{\n        \
             return nil, err\n    \
         }}\n    \
         return &out, nil"
    )
}

fn gen_java_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> String {
    let name = &adapter.name;
    let prefix = config.ffi_prefix();
    let owner_type = adapter.owner_type.as_deref().unwrap_or("Client");
    let owner_snake = to_snake_case(owner_type);

    let arg_pass = if adapter.params.is_empty() {
        String::new()
    } else {
        format!(
            ", {}",
            adapter
                .params
                .iter()
                .map(|p| {
                    if p.ty == "String" || p.ty == "&str" {
                        format!("arena.allocateFrom({})", p.name)
                    } else {
                        p.name.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    format!(
        "try (var arena = Arena.ofConfined()) {{\n\
         \x20           var result = (MemorySegment) {prefix}_{owner_snake}_{name}.invokeExact(this.handle, arena{arg_pass});\n\
         \x20           if (result.equals(MemorySegment.NULL)) {{\n\
         \x20               throw new RuntimeException(lastError());\n\
         \x20           }}\n\
         \x20           return result.getString(0);\n\
         \x20       }}"
    )
}

fn gen_csharp_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> String {
    let name = &adapter.name;
    let prefix = config.ffi_prefix();
    let owner_type = adapter.owner_type.as_deref().unwrap_or("Client");
    let owner_snake = to_snake_case(owner_type);

    let call_args_list: Vec<String> = adapter.params.iter().map(|p| p.name.clone()).collect();
    let call_str = call_args_list.join(", ");
    let call_pass = if call_str.is_empty() {
        String::new()
    } else {
        format!(", {}", call_str)
    };

    format!(
        "var ptr = {prefix}_{owner_snake}_{name}_native(this.handle{call_pass});\n\
         \x20       if (ptr == IntPtr.Zero)\n\
         \x20           throw new InvalidOperationException(GetLastError());\n\
         \x20       try {{ return Marshal.PtrToStringUTF8(ptr)!; }}\n\
         \x20       finally {{ FreeString(ptr); }}"
    )
}

fn gen_r_body(adapter: &AdapterConfig, _config: &ResolvedCrateConfig) -> anyhow::Result<String> {
    let raw_returns = adapter.returns.as_deref().unwrap_or("Robj");

    let map_expr = if raw_returns == "bytes::Bytes" {
        ".map(|b| b.to_vec())".to_string()
    } else {
        format!(".map({raw_returns}::from)")
    };

    let args = call_args(adapter);
    let core_call =
        super::rust_callable::receiver_call(&adapter.core_path, "self.inner", "&self.inner", &args.join(", "))?;

    Ok(crate::codegen::template_env::render(
        "adapters/async_r_body.rs.jinja",
        crate::alef_context! { core_call => core_call, map_expr => map_expr },
    )
    .trim_end()
    .to_owned())
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.extend(ch.to_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
pub(crate) mod callable_test_support;
#[cfg(test)]
pub(crate) mod callable_tests;

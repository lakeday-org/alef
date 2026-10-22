use super::{call_args, core_let_bindings_cloned, php_stream_handle_name};
use crate::core::config::{AdapterConfig, ResolvedCrateConfig};

pub(super) fn gen_php_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> (String, Option<String>) {
    let core_import = config.core_import_name();
    let item_type = source_type(adapter.item_type.as_deref().unwrap_or("()"), &core_import);
    let error_type = source_type(adapter.error_type.as_deref().unwrap_or("anyhow::Error"), &core_import);
    let handle = php_stream_handle_name(adapter);
    let namespace = crate::backends::php::naming::php_autoload_namespace(config);
    let function = crate::codegen::naming::abi_symbol_from_components([
        "alef_stream",
        adapter.owner_type.as_deref().unwrap_or(""),
        &adapter.name,
    ]);
    let context = crate::alef_context! {
        core_path => &adapter.core_path,
        namespace => &namespace,
        function => &function,
        function_name => format!("{namespace}\\{function}"),
        class_name => format!("{namespace}\\{handle}"),
        handle => &handle,
        stream_type => format!("futures_util::stream::BoxStream<'static, std::result::Result<{item_type}, {error_type}>>"),
        call_args => call_args(adapter).join(", "),
        let_bindings => core_let_bindings_cloned(adapter, &core_import),
    };
    let body = crate::backends::php::template_env::render("php_stream_body.rs.jinja", context.clone());
    let handle = crate::backends::php::template_env::render("php_stream_handle.rs.jinja", context);
    (body.trim_end().to_string(), Some(handle.trim_end().to_string()))
}

fn source_type(name: &str, core_import: &str) -> String {
    if name.contains("::")
        || matches!(
            name,
            "()" | "String"
                | "bool"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "u128"
                | "i128"
                | "usize"
                | "isize"
                | "char"
                | "f32"
                | "f64"
        )
    {
        name.to_string()
    } else {
        format!("{core_import}::{name}")
    }
}

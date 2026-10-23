use super::{call_args, core_let_bindings_cloned, php_stream_handle_name, source_type};
use crate::core::config::{AdapterConfig, ResolvedCrateConfig};

pub(super) fn gen_php_body(
    adapter: &AdapterConfig,
    config: &ResolvedCrateConfig,
) -> anyhow::Result<(String, Option<String>)> {
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
        core_call => crate::adapters::rust_callable::receiver_call(&adapter.core_path, "self.inner", "&self.inner", &call_args(adapter).join(", "))?,
        namespace => &namespace,
        function => &function,
        function_name => format!("{namespace}\\{function}"),
        class_name => format!("{namespace}\\{handle}"),
        handle => &handle,
        return_type => crate::backends::php::STREAM_RESULT_TYPE,
        stream_type => format!("futures_util::stream::BoxStream<'static, std::result::Result<{item_type}, {error_type}>>"),
        call_args => call_args(adapter).join(", "),
        let_bindings => core_let_bindings_cloned(adapter, &core_import),
    };
    let body = crate::backends::php::template_env::render("php_stream_body.rs.jinja", context.clone());
    let handle = crate::backends::php::template_env::render("php_stream_handle.rs.jinja", context);
    Ok((body.trim_end().to_string(), Some(handle.trim_end().to_string())))
}

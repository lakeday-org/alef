use crate::codegen::naming::{internal_rust_identifier, pascal_to_snake, to_class_name};
use crate::core::config::{AdapterConfig, ResolvedCrateConfig};

pub(crate) fn generate_body(
    adapter: &AdapterConfig,
    config: &ResolvedCrateConfig,
    typed_error: bool,
) -> anyhow::Result<(String, Option<String>)> {
    let body = crate::backends::dart::template_env::render(
        "rust_streaming_body.rs.jinja",
        crate::alef_context! {
            core_call => crate::adapters::rust_callable::receiver_call(&adapter.core_path, "inner", "&inner", &super::call_args(adapter).join(", "))?,
            item_type => adapter.item_type.as_deref().unwrap_or("()"),
            call_args => super::call_args(adapter).join(", "),
            bindings => super::core_let_bindings(adapter, &config.core_import_name()),
            transport => typed_error.then(|| transport_names(adapter)),
        },
    );
    Ok((body, None))
}

#[derive(serde::Serialize)]
pub(crate) struct StreamTransport {
    pub carrier: String,
    pub wire: String,
    pub module: String,
    pub converter: String,
    pub decoder: String,
    pub unit_item: bool,
}

pub(crate) fn transport_names(adapter: &AdapterConfig) -> StreamTransport {
    let owner = adapter.owner_type.as_deref().unwrap_or("Module");
    let stem = to_class_name(&format!("{owner}_{}", adapter.name));
    let module = internal_rust_identifier(&format!("alef_stream_{}", pascal_to_snake(&stem)));
    StreamTransport {
        carrier: internal_rust_identifier(&format!("AlefStream{stem}")),
        wire: internal_rust_identifier(&format!("AlefStream{stem}Wire")),
        converter: internal_rust_identifier(&format!("alef_encode_stream_{}", pascal_to_snake(&stem))),
        decoder: internal_rust_identifier(&format!("alef_decode_stream_{}", pascal_to_snake(&stem))),
        unit_item: syn::parse_str::<syn::Type>(adapter.item_type.as_deref().unwrap_or("()"))
            .is_ok_and(|ty| matches!(ty, syn::Type::Tuple(tuple) if tuple.elems.is_empty())),
        module,
    }
}

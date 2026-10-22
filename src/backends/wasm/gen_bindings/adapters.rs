use crate::backends::wasm::type_map::WasmMapper;
use crate::codegen::type_mapper::TypeMapper;
use crate::core::config::{AdapterConfig, AdapterPattern, Language, ResolvedCrateConfig};
use crate::core::ir::TypeRef;

pub(crate) fn generate_body(adapter: &AdapterConfig, config: &ResolvedCrateConfig) -> String {
    let overrides = config
        .wasm
        .as_ref()
        .map(|wasm| wasm.type_overrides.clone())
        .unwrap_or_default();
    let mapper = WasmMapper::new(overrides, config.wasm_type_prefix());
    body_with_mapper(adapter, &mapper)
}

pub(super) fn build_bodies(
    config: &ResolvedCrateConfig,
    mapper: &WasmMapper,
) -> anyhow::Result<crate::adapters::AdapterBodies> {
    let mut bodies = crate::adapters::build_adapter_bodies(config, Language::Wasm)?;
    for adapter in &config.adapters {
        if !matches!(
            adapter.pattern,
            AdapterPattern::AsyncMethod | AdapterPattern::SyncFunction
        ) {
            continue;
        }
        let key = match &adapter.owner_type {
            Some(owner) => format!("{owner}.{}", adapter.name),
            None => adapter.name.clone(),
        };
        bodies.insert(key, body_with_mapper(adapter, mapper));
    }
    Ok(bodies)
}

fn body_with_mapper(adapter: &AdapterConfig, mapper: &WasmMapper) -> String {
    let raw = adapter.returns.as_deref().unwrap_or("JsValue");
    let map_function = if raw == "bytes::Bytes" {
        Some("|result| result.to_vec()".to_string())
    } else if raw == "JsValue" {
        None
    } else {
        let ty = syn::parse_str::<syn::Type>(raw)
            .map(|ty| crate::extract::type_resolver::resolve_type(&ty))
            .unwrap_or_else(|_| TypeRef::Named(raw.to_string()));
        (convert_return("value", &ty, mapper) != "value").then(|| conversion_function(&ty, mapper))
    };
    crate::backends::wasm::template_env::render(
        "adapter_result_body",
        crate::alef_context! {
            core_path => &adapter.core_path,
            arguments => match adapter.pattern {
                AdapterPattern::AsyncMethod => crate::adapters::async_method::call_args(adapter),
                _ => crate::adapters::sync_function::call_args(adapter),
            }.join(", "),
            is_async => matches!(adapter.pattern, AdapterPattern::AsyncMethod),
            map_function => map_function,
        },
    )
}

fn convert_return(expr: &str, ty: &TypeRef, mapper: &WasmMapper) -> String {
    let mapped = mapper.map_type(ty);
    if let Some(wrapped) = super::functions::wrap_jsvalue_mapped_return(expr, &mapped) {
        return wrapped;
    }
    match ty {
        TypeRef::Named(_) => format!("{mapped}::from({expr})"),
        TypeRef::Optional(inner) => format!("{expr}.map({})", conversion_function(inner, mapper)),
        TypeRef::Vec(inner) => format!(
            "{expr}.into_iter().map({}).collect()",
            conversion_function(inner, mapper)
        ),
        _ => expr.to_string(),
    }
}

fn conversion_function(ty: &TypeRef, mapper: &WasmMapper) -> String {
    let mapped = mapper.map_type(ty);
    if matches!(ty, TypeRef::Named(_)) && !crate::codegen::shared::maps_to_js_value(&mapped) {
        format!("{mapped}::from")
    } else {
        format!("|value| {}", convert_return("value", ty, mapper))
    }
}

use super::named_type_name;
use crate::core::ir::{FunctionDef, TypeRef};

pub(super) fn helper_params(func: &FunctionDef) -> String {
    let mut params = func
        .params
        .iter()
        .map(|param| {
            let go_name = crate::codegen::naming::go_param_name(&param.name);
            let go_type: String = if param.optional {
                crate::backends::go::type_map::go_optional_type(&param.ty).into_owned()
            } else if let TypeRef::Named(_) = &param.ty {
                format!("*{}", crate::backends::go::type_map::go_type(&param.ty))
            } else {
                crate::backends::go::type_map::go_type(&param.ty).into_owned()
            };
            format!("{go_name} {go_type}")
        })
        .collect::<Vec<_>>();
    params.push("visitor Visitor".to_string());
    params.join(", ")
}

pub(crate) fn visitor_helper_name(function_name: &str) -> String {
    format!(
        "{}WithVisitorHelper",
        crate::codegen::naming::go_param_name(function_name)
    )
}

pub(super) fn helper_c_param_setup(func: &FunctionDef, options_type: &str) -> String {
    let mut out = String::new();
    for param in &func.params {
        if named_type_name(&param.ty) == Some(options_type) {
            continue;
        }
        if matches!(param.ty, TypeRef::String | TypeRef::Path) {
            let go_name = crate::codegen::naming::go_param_name(&param.name);
            let c_name = crate::codegen::naming::go_param_name(&format!("c_{}", param.name));
            out.push_str(&crate::backends::go::template_env::render(
                "c_string_arg_setup.jinja",
                crate::alef_context! {
                    c_name => &c_name,
                    go_name => &go_name,
                },
            ));
        } else if let Some(setup) = crate::backends::go::gen_bindings::visitor_scalar_param_to_c(param) {
            out.push_str(&setup);
        }
    }
    out
}

pub(super) fn helper_call_args(func: &FunctionDef, options_type: &str) -> String {
    func.params
        .iter()
        .map(|param| {
            if named_type_name(&param.ty) == Some(options_type) {
                "cOptions".to_string()
            } else if matches!(param.ty, TypeRef::String | TypeRef::Path)
                || crate::backends::go::gen_bindings::is_visitor_scalar_param(param)
            {
                crate::codegen::naming::go_param_name(&format!("c_{}", param.name))
            } else {
                crate::codegen::naming::go_param_name(&param.name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn supports_options_visitor_function(func: &FunctionDef, options_type: &str) -> bool {
    !func.is_async
        && !func.binding_excluded
        && named_type_name(&func.return_type).is_some()
        && func
            .params
            .iter()
            .any(|param| named_type_name(&param.ty) == Some(options_type))
}

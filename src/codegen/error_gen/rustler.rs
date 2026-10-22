use crate::core::ir::ErrorDef;

use super::shared::{duration_methods, error_variant_wildcard_pattern, millis_expr, to_snake_case};

/// Core path of an error type as the generated NIF crate names it.
fn core_error_path(error: &ErrorDef, core_import: &str) -> String {
    if error.rust_path.is_empty() {
        format!("{core_import}::{}", error.name)
    } else {
        error.rust_path.replace('-', "_")
    }
}

/// Errors the Elixir binding exposes as exception structs, in a fixed order.
pub fn rustler_typed_errors(errors: &[ErrorDef]) -> Vec<&ErrorDef> {
    let mut typed: Vec<&ErrorDef> = errors.iter().filter(|error| !error.binding_excluded).collect();
    typed.sort_by(|a, b| a.name.cmp(&b.name));
    typed
}

/// Name of the Elixir exception module for `error`, below the app module.
///
/// The app prefix is dropped from the error name. When two errors would share a name,
/// the full error name is kept.
pub fn rustler_error_module_name(error: &ErrorDef, app_module: &str, errors: &[&ErrorDef]) -> String {
    let short = match error.name.strip_prefix(app_module) {
        Some(rest) if !rest.is_empty() => rest.to_string(),
        _ => error.name.clone(),
    };
    let taken =
        errors
            .iter()
            .filter(|other| other.name != error.name)
            .any(|other| match other.name.strip_prefix(app_module) {
                Some(rest) if !rest.is_empty() => rest == short,
                _ => other.name == short,
            });
    if taken { error.name.clone() } else { short }
}

/// Name of the function that converts a core error into the NIF error.
pub fn rustler_converter_fn_name(error: &ErrorDef) -> String {
    format!("{}_to_nif_err", to_snake_case(&error.name))
}

/// Generate a converter that maps a core error to `AlefNifError::Typed`.
///
/// The kind is the snake_case variant name. The status is the error's `status_code` method
/// (0 when the error has none). The retry delay is the error's `retry_after` method in whole
/// milliseconds (absent when the error has none).
pub fn gen_rustler_error_converter(error: &ErrorDef, core_import: &str, exception_module: &str) -> String {
    let rust_path = core_error_path(error, core_import);
    let fn_name = rustler_converter_fn_name(error);

    let arms = error
        .variants
        .iter()
        .map(|variant| {
            (
                error_variant_wildcard_pattern(&rust_path, variant),
                to_snake_case(&variant.name),
            )
        })
        .collect::<Vec<_>>();

    let status = if error.methods.iter().any(|m| m.name == "status_code") {
        "e.status_code()"
    } else {
        "0"
    };
    let retry = match duration_methods(error)
        .into_iter()
        .find(|(m, _)| m.name == "retry_after")
    {
        Some((_, true)) => millis_expr("e.retry_after()", true, "u64"),
        Some((_, false)) => format!("Some({})", millis_expr("e.retry_after()", false, "u64")),
        None => "None".to_string(),
    };

    crate::codegen::template_env::render(
        "error_gen/rustler_error_converter.jinja",
        crate::alef_context! {
            rust_path => rust_path, fn_name => fn_name, arms => arms,
            exception_module => exception_module, status => status, retry => retry,
        },
    )
}

/// Generate `AlefNifError` and `alef_nif_err`, the one conversion every NIF call site uses.
///
/// A core error that has a typed converter keeps its kind, status and retry delay and reaches
/// Elixir as an exception struct. Any other `Display` error stays a plain message string.
pub fn gen_rustler_error_support(errors: &[&ErrorDef], core_import: &str) -> String {
    let arms = errors
        .iter()
        .map(|error| (core_error_path(error, core_import), rustler_converter_fn_name(error)))
        .collect::<Vec<_>>();
    crate::codegen::template_env::render(
        "error_gen/rustler_error_support.jinja",
        crate::alef_context! { arms => arms },
    )
}

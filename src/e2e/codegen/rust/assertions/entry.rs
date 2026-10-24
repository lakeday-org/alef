use super::{Assertion, FieldResolver, render_assertion_with_streaming};

#[allow(clippy::too_many_arguments)]
pub fn render_assertion(
    out: &mut String,
    assertion: &Assertion,
    result_var: &str,
    module: &str,
    dep_name: &str,
    is_error_context: bool,
    unwrapped_fields: &[(String, String)], // (fixture_field, local_var)
    field_resolver: &FieldResolver,
    result_is_tree: bool,
    result_is_simple: bool,
    result_is_vec: bool,
    result_is_option: bool,
    returns_result: bool,
    streaming_item_type: Option<&str>,
) {
    render_assertion_with_streaming(
        out,
        assertion,
        result_var,
        module,
        dep_name,
        is_error_context,
        unwrapped_fields,
        field_resolver,
        result_is_tree,
        result_is_simple,
        result_is_vec,
        result_is_option,
        returns_result,
        streaming_item_type,
        false,
    )
}

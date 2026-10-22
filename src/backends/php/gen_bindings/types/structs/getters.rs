use crate::codegen::builder::ImplBuilder;
use crate::codegen::naming::{PublicIdentifierKind, public_host_identifier};
use crate::core::config::Language;

pub(super) const JSON_TEXT_EXPR: &str = "serde_json::to_value(v).ok().and_then(|json| match json { serde_json::Value::String(text) => Some(text), other => serde_json::to_string(&other).ok() })";

pub(super) fn add_php_getter(
    builder: &mut ImplBuilder,
    getter_ident: &str,
    field_name: &str,
    ret: &str,
    body: &str,
    as_property: bool,
) {
    let method = public_host_identifier(
        Language::Php,
        PublicIdentifierKind::Method,
        &format!("get_{field_name}"),
    );
    builder.add_method(&crate::backends::php::template_env::render(
        "php_struct_field_getter.jinja",
        crate::alef_context! {
            getter_ident => getter_ident,
            field_name => field_name,
            return_type => ret,
            body => body,
            as_property => as_property,
            method => method,
        },
    ));
}

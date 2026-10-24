use crate::core::ir::TypeRef;
use std::collections::HashSet;

pub(super) fn array_suffix(ty: &TypeRef, dto_names: &HashSet<String>, class_names: &HashSet<String>) -> Option<String> {
    let TypeRef::Vec(inner) = ty else { return None };
    let (conversion, throws) = match inner.as_ref() {
        TypeRef::String => ("$0.toString()".to_string(), false),
        TypeRef::Named(name) if dto_names.contains(name) => {
            (format!("{}($0)", super::overloads::swift_type_name(inner)), true)
        }
        TypeRef::Named(name) if class_names.contains(name) => {
            (format!("{}($0)", super::overloads::swift_type_name(inner)), false)
        }
        TypeRef::Named(_) => return Some(String::new()),
        _ => return None,
    };
    Some(
        crate::backends::swift::template_env::render(
            "swift_owned_array_suffix.swift.jinja",
            crate::alef_context! { conversion => conversion, throws => throws },
        )
        .trim()
        .to_string(),
    )
}

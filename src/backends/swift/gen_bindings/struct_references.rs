pub(super) fn emit(type_names: &[String], body: &mut String) {
    for type_name in type_names {
        body.push_str(&crate::backends::swift::template_env::render(
            "typealias.jinja",
            crate::alef_context! {
                name => &format!("{type_name}Ref"),
            },
        ));
        body.push('\n');
        body.push_str(&crate::backends::swift::template_env::render(
            "typealias.jinja",
            crate::alef_context! {
                name => &format!("{type_name}RefMut"),
            },
        ));
        body.push('\n');
    }
}

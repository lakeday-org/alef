use super::{parse_author, render};

pub(super) fn render_developers(authors: &[String]) -> String {
    if authors.is_empty() {
        return "\n".to_owned();
    }
    let authors = authors
        .iter()
        .map(|author| {
            let (name, email) = parse_author(author);
            crate::alef_context! { name => literal(name), email => literal(email) }
        })
        .collect::<Vec<_>>();
    render("pom_developers.jinja", crate::alef_context! { authors => authors })
}

fn literal(value: &str) -> String {
    crate::codegen::coordinates::kotlin_string_escape(value)
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

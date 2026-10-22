use std::collections::HashSet;

use super::{PublicIdentifierKind, pascal_to_snake, public_host_identifier};
use crate::core::config::Language;

const FIRST_COLLISION_SUFFIX: usize = 2;

/// Allocate one public Python payload getter against its class member namespace. ~keep
pub(crate) fn python_enum_payload_getter_name(
    variant: &str,
    has_bare_constructor: bool,
    occupied: &HashSet<String>,
) -> String {
    let snake = pascal_to_snake(variant);
    let preferred = if has_bare_constructor {
        format!("{snake}_payload")
    } else {
        snake
    };
    let mut preferred = public_host_identifier(Language::Python, PublicIdentifierKind::Field, &preferred);
    if matches!(preferred.as_str(), "self" | "crate" | "super") {
        preferred.push('_');
    }
    let mut candidate = preferred.clone();
    let mut suffix = FIRST_COLLISION_SUFFIX;
    while occupied.contains(&candidate) {
        candidate = format!("{preferred}_{suffix}");
        suffix += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::python_enum_payload_getter_name;

    #[test]
    fn python_payload_names_reserve_sibling_members_and_escape_keywords() {
        for (variant, constructor, reserved, expected) in [
            ("Text", true, vec![], "text_payload"),
            ("Text", true, vec!["text_payload"], "text_payload_2"),
            ("Text", true, vec!["text_payload", "text_payload_2"], "text_payload_3"),
            ("Class", false, vec![], "class_"),
            ("Self", false, vec![], "self_"),
            ("Crate", false, vec![], "crate_"),
            ("Super", false, vec![], "super_"),
            ("Type", false, vec![], "type_"),
        ] {
            let occupied = reserved.into_iter().map(String::from).collect();
            assert_eq!(
                python_enum_payload_getter_name(variant, constructor, &occupied),
                expected,
                "{variant}: {occupied:?}"
            );
        }
    }
}

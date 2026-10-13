//! Dart wrappers over the shared C component-manager ABI.

pub(super) fn emit(prefix: &str, out: &mut String) {
    out.push_str(&crate::backends::dart::template_env::render(
        "ffi_components.jinja",
        crate::alef_context! { prefix => prefix },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_owned_string_and_mobile_safe_component_api() {
        let mut generated = String::new();
        emit("demo_core", &mut generated);

        assert!(generated.contains("demo_core_component_load"));
        assert!(generated.contains("demo_core_component_prefetch"));
        assert!(generated.contains("_freeString(pointer.cast<Char>())"));
        assert!(generated.contains("Platform.isAndroid || Platform.isIOS"));
        assert!(generated.contains("jsonDecode(json)"));
        // Preserve output bytes while moving component structure into Jinja. ~keep
        use sha2::Digest as _;
        assert_eq!(
            sha2::Sha256::digest(generated.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "cbfe1d1ba1775d94e0eca401b94d8fb3f968db1fef4511e2674012c6a6a8e2ab"
        );
    }
}

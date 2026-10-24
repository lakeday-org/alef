//! Swift-specific naming helpers for `ResolvedCrateConfig`.

use crate::core::config::ResolvedCrateConfig;
use crate::core::template_versions;

/// Return the canonical Swift protocol name for a trait bridge.
///
/// Both the production wrapper codegen (`gen_bindings/trait_bridge.rs`) and the
/// e2e stub emitter (`e2e/codegen/swift.rs`) must call this function so that a
/// rename in one place is automatically reflected in the other.
///
/// # Example
/// ```
/// use alef::backends::swift::naming::bridge_protocol_name;
/// assert_eq!(bridge_protocol_name("OcrBackend"), "SwiftOcrBackendBridge");
/// ```
pub fn bridge_protocol_name(trait_name: &str) -> String {
    format!("Swift{trait_name}Bridge")
}

/// Resolve a Swift public source identifier.
pub(crate) fn swift_source_ident(name: &str) -> String {
    crate::codegen::naming::escape_identifier_for(
        crate::core::config::Language::Swift,
        name,
        crate::codegen::naming::IdentifierContext::SwiftSource,
    )
}

/// Resolve a Swift Rust-shim identifier.
pub(crate) fn swift_rust_shim_ident(name: &str) -> String {
    crate::codegen::naming::escape_identifier_for(
        crate::core::config::Language::Swift,
        name,
        crate::codegen::naming::IdentifierContext::SwiftRustShim,
    )
}

pub(crate) fn text_value_accessor_name(field: &str, renamed: Option<&str>) -> String {
    crate::codegen::naming::public_host_identifier(
        crate::core::config::Language::Swift,
        crate::codegen::naming::PublicIdentifierKind::Method,
        &format!("{}_value", renamed.unwrap_or(field)),
    )
}

pub(crate) fn bridge_field_name(field: &str) -> String {
    let name = crate::codegen::naming::public_host_identifier(
        crate::core::config::Language::Swift,
        crate::codegen::naming::PublicIdentifierKind::Method,
        field,
    );
    swift_rust_shim_ident(name.trim_matches('`'))
}

pub(crate) fn owned_vector_raw_name(public_name: &str) -> String {
    crate::codegen::naming::internal_rust_identifier(&format!(
        "__alef_owned_vec_{}",
        crate::codegen::naming::pascal_to_snake(public_name.trim_matches('`')),
    ))
}

/// Stable internal symbols follow the explicitly recorded constructor chronology. ~keep
pub(crate) fn constructor_history_symbol(index: usize) -> String {
    crate::codegen::naming::internal_rust_identifier(&format!("__alef_constructor_history_{index}"))
}

/// Get the swift-bridge version to pin in the generated `Cargo.toml`.
///
/// Returns the per-crate override from `[crates.swift] swift_bridge_version` when set;
/// otherwise falls back to the compiled-in default constant.
pub fn swift_bridge_version(config: &ResolvedCrateConfig) -> String {
    config
        .swift
        .as_ref()
        .and_then(|s| s.swift_bridge_version.as_ref())
        .cloned()
        .unwrap_or_else(|| template_versions::cargo::SWIFT_BRIDGE.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::new_config::NewAlefConfig;
    use crate::core::template_versions;

    #[test]
    fn constructor_history_symbols_keep_their_reserved_internal_namespace() {
        for (index, expected) in [(1, "__alef_constructor_history_1"), (2, "__alef_constructor_history_2")] {
            assert_eq!(constructor_history_symbol(index), expected);
        }
    }

    #[test]
    fn owned_vector_aliases_use_canonical_member_policy() {
        for (field, getter, raw) in [
            ("items", "items", "__alef_owned_vec_items"),
            ("URL_items", "urlItems", "__alef_owned_vec_url_items"),
            ("class", "class_", "__alef_owned_vec_class"),
        ] {
            assert_eq!(bridge_field_name(field), getter);
            assert_eq!(owned_vector_raw_name(field), raw);
        }
    }

    fn resolved_one(toml: &str) -> ResolvedCrateConfig {
        let cfg: NewAlefConfig = toml::from_str(toml).unwrap();
        cfg.resolve().unwrap().remove(0)
    }

    fn minimal() -> ResolvedCrateConfig {
        resolved_one(
            r#"
[workspace]
languages = ["swift"]

[[crates]]
name = "test-lib"
sources = ["src/lib.rs"]
"#,
        )
    }

    #[test]
    fn swift_bridge_version_defaults_to_constant() {
        let r = minimal();
        assert_eq!(
            swift_bridge_version(&r),
            template_versions::cargo::SWIFT_BRIDGE,
            "should fall back to compiled-in default"
        );
    }

    #[test]
    fn swift_bridge_version_explicit_override_wins() {
        let r = resolved_one(
            r#"
[workspace]
languages = ["swift"]

[[crates]]
name = "test-lib"
sources = ["src/lib.rs"]

[crates.swift]
swift_bridge_version = "0.2.0"
"#,
        );
        assert_eq!(
            swift_bridge_version(&r),
            "0.2.0",
            "explicit swift_bridge_version override should win"
        );
    }
}

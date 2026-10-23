#[path = "typed_variant_list_native_support.rs"]
pub(crate) mod typed_variant_list_native_support;
#[path = "typed_variant_list_test_support.rs"]
pub(crate) mod typed_variant_list_test_support;

#[cfg(test)]
mod preserved_url_tests {
    use super::super::{preserved_url_list, preserved_url_literal};

    #[test]
    fn scalar_url_is_preserved_only_when_requested() {
        let value = serde_json::json!("http://127.0.0.1/private");
        assert_eq!(preserved_url_literal(true, &value), Some("http://127.0.0.1/private"));
        assert_eq!(preserved_url_literal(false, &value), None);
    }

    #[test]
    fn url_list_is_preserved_atomically() {
        let value = serde_json::json!(["http://host-a.test/", "file:///tmp/example"]);
        assert_eq!(
            preserved_url_list(true, &value),
            Some(vec!["http://host-a.test/", "file:///tmp/example"])
        );
        assert_eq!(
            preserved_url_list(true, &serde_json::json!(["https://host.test", 7])),
            None
        );
    }
}

#[cfg(test)]
mod unimplemented_test_backend_tests {
    use super::super::emit_test_backend;
    use crate::core::config::TraitBridgeConfig;
    use crate::e2e::fixture::Fixture;

    fn registered_bridge() -> TraitBridgeConfig {
        TraitBridgeConfig {
            trait_name: "SampleBackend".into(),
            ..TraitBridgeConfig::default()
        }
    }

    fn sample_fixture() -> Fixture {
        Fixture {
            id: "register_sample_backend".into(),
            ..Fixture::default()
        }
    }

    /// Every language with no real `test_backend` stub generator (including a
    /// language string the dispatch `match` doesn't even recognize) must panic
    /// through the public dispatcher rather than hand back a placeholder
    /// `TestBackendEmission`. There is no sentinel value left to construct — the
    /// `unimplemented()` constructor and `UNIMPLEMENTED_MARKER` were removed —
    /// so this is the structural proof that a caller can never receive a
    /// stand-in emission for these targets, whether or not the trait is
    /// registered: a language with no real generator fails the same way
    /// regardless of `trait_bridges` config.
    #[test]
    fn languages_without_a_real_emitter_panic_through_the_dispatcher() {
        for language in ["gleam", "brew", "php_ext", "homebrew", "kotlin", "not-a-real-language"] {
            let bridge = registered_bridge();
            let fixture = sample_fixture();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                emit_test_backend(language, &bridge, &[], &fixture, &[], "")
            }));
            assert!(
                result.is_err(),
                "expected `{language}` to panic instead of returning a TestBackendEmission, but it returned a value"
            );
        }
    }
}

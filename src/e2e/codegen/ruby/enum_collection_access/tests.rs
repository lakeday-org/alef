use std::collections::{HashMap, HashSet};

use crate::e2e::config::E2eConfig;

use crate::e2e::codegen::php::enum_collection_test_support as support;

#[test]
fn ruby_enum_collection_assertions_include_count_and_both_text_checks() {
    rendered_assertions();
}

#[test]
#[ignore = "native Ruby proof requires RSpec matchers"]
fn ruby_enum_collection_assertions_execute_count_and_both_text_checks() {
    support::verify_native("ruby", "rb", &rendered_assertions());
}

fn rendered_assertions() -> String {
    let surface = support::surface();
    let resolver = support::resolver(&surface, "ruby").with_ruby_hash_serialized_enum_names(
        super::super::enum_variant_access::hash_serialized_enum_names(&surface.enums),
    );
    let mut rendered = String::new();
    for assertion in support::assertions() {
        super::super::assertions::render_assertion(
            &mut rendered,
            &assertion,
            "result",
            &resolver,
            false,
            &E2eConfig::default(),
            &HashSet::new(),
            &HashMap::new(),
        );
    }
    assert_eq!(rendered.matches("expect(").count(), 3, "{rendered}");
    assert!(!rendered.to_lowercase().contains("skipped"), "{rendered}");
    assert!(rendered.contains(".fetch('text'.to_sym)"), "{rendered}");
    rendered
}

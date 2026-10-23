use std::collections::{BTreeMap, HashSet};

use super::super::enum_variant_access::{PhpEnumLowering, PhpVariantAccess};
use crate::core::config::e2e::CallConfig;

use super::test_support as support;

#[test]
fn php_enum_collection_assertions_execute_count_and_both_text_checks() {
    let surface = support::surface();
    let fields = HashSet::from(["choices".to_string()]);
    let getters =
        super::super::types::build_php_getter_map(&surface.types, &surface.enums, &CallConfig::default(), &fields);
    let resolver = support::resolver(&surface, "php").with_php_getter_map(getters.clone());
    let lowering = PhpEnumLowering::from_enums(&surface.enums);
    let mut rendered = String::new();
    for assertion in support::assertions() {
        super::super::assertions::render_assertion(
            &mut rendered,
            &assertion,
            "result",
            &resolver,
            false,
            false,
            &BTreeMap::new(),
            false,
            &PhpVariantAccess::new(&getters, &lowering),
        );
    }
    assert_eq!(rendered.matches("$this->assert").count(), 3, "{rendered}");
    assert!(!rendered.to_lowercase().contains("skipped"), "{rendered}");
    assert!(rendered.contains("->getContent()"), "{rendered}");
    support::verify_native("php", "php", &rendered);
}

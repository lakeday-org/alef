use alef::e2e::field_access::SwiftFirstClassMap;

#[test]
fn existing_external_swift_map_literal_remains_source_compatible() {
    let map = SwiftFirstClassMap {
        first_class_types: Default::default(),
        field_types: Default::default(),
        vec_field_names: Default::default(),
        json_bridged_field_names: Default::default(),
        json_bridged_by_type: Default::default(),
        getter_optionality: Default::default(),
        root_type: None,
        stringy_fields_by_type: Default::default(),
    };
    assert!(!map.is_first_class(Some("Sample")));
}

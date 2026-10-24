use crate::e2e::field_access::FieldResolver;
use crate::e2e::fixture::Assertion;
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
pub(in crate::e2e::codegen::swift) fn render_assertion(
    out: &mut String,
    assertion: &Assertion,
    result_var: &str,
    field_resolver: &FieldResolver,
    result_is_simple: bool,
    result_is_array: bool,
    result_is_option: bool,
    result_element_is_string: bool,
    result_field_accessor: &HashMap<String, String>,
    is_streaming: bool,
    returns_void: bool,
    module_name: &str,
) {
    super::render_assertion_with_usage_accessors(
        out,
        assertion,
        result_var,
        field_resolver,
        result_is_simple,
        result_is_array,
        result_is_option,
        result_element_is_string,
        result_field_accessor,
        is_streaming,
        returns_void,
        module_name,
        None,
    );
}

#[test]
fn usage_accessors_do_not_reinterpret_array_element_accessor_configuration() {
    use std::collections::HashSet;
    let resolver = FieldResolver::new(
        &HashMap::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
    );
    let assertion = Assertion {
        assertion_type: "is_empty".into(),
        field: Some("tool_calls".into()),
        ..Default::default()
    };
    let array_accessors = HashMap::from([("tool_calls".into(), "function".into())]);
    let usage_accessors = HashMap::from([("usage.total_tokens".into(), "chunks.last?.usage?.totalTokens".into())]);
    let mut source = String::new();
    super::render_assertion_with_usage_accessors(
        &mut source,
        &assertion,
        "result",
        &resolver,
        false,
        false,
        false,
        false,
        &array_accessors,
        true,
        false,
        "Sample",
        Some(&usage_accessors),
    );
    assert!(source.contains("flatMap"), "{source}");
    assert!(!source.contains("XCTAssertTrue(function.isEmpty"), "{source}");
}

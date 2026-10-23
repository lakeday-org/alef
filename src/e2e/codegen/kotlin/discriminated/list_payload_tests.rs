use crate::e2e::codegen::test_support::typed_variant_list_test_support as support;

fn render(android: bool, enum_elements: bool, wire_rename: bool) -> String {
    let resolver = support::resolver(
        if android { "kotlin_android" } else { "kotlin" },
        enum_elements,
        wire_rename,
    );
    let mut source = String::new();
    for assertion in support::assertions(wire_rename) {
        assert!(super::try_render_generic_union_assertion(
            &mut source,
            &assertion,
            &resolver,
            "result",
            android,
            assertion.field.as_deref().unwrap()
        ));
    }
    source
}

#[test]
fn indexed_list_variant_assertions_check_count_and_exact_text() {
    for (enum_elements, wire_rename) in [(false, false), (true, false), (true, true)] {
        for android in [false, true] {
            support::verify(&render(android, enum_elements, wire_rename), ".size");
        }
    }
}

use crate::e2e::codegen::test_support::typed_variant_list_native_support as native;

#[test]
#[ignore = "requires target compiler, assertion framework and generated binding dependencies"]
fn indexed_list_variant_assertions_run_against_actual_generated_types() {
    for enum_elements in [false, true] {
        for android in [false, true] {
            let language = if android { "kotlin_android" } else { "kotlin" };
            native::run(language, enum_elements, &render(android, enum_elements, enum_elements));
        }
    }
}

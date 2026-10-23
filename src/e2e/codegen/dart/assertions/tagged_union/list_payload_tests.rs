use crate::e2e::codegen::test_support::typed_variant_list_test_support as support;

fn render(enum_elements: bool, wire_rename: bool) -> String {
    let resolver = support::resolver("dart", enum_elements, wire_rename);
    let mut source = String::new();
    for assertion in support::assertions(wire_rename) {
        assert!(super::try_render_tagged_union_assertion(
            &mut source,
            &assertion,
            "result",
            &resolver,
            assertion.field.as_deref().unwrap()
        ));
    }
    source
}

#[test]
fn indexed_list_variant_assertions_check_count_and_exact_text() {
    for (enum_elements, wire_rename) in [(false, false), (true, false), (true, true)] {
        support::verify(&render(enum_elements, wire_rename), ".length");
    }
}

use crate::e2e::codegen::test_support::typed_variant_list_native_support as native;

#[test]
#[ignore = "requires target compiler, assertion framework and generated binding dependencies"]
fn indexed_list_variant_assertions_run_against_actual_generated_types() {
    for enum_elements in [false, true] {
        native::run("dart", enum_elements, &render(enum_elements, enum_elements));
    }
}

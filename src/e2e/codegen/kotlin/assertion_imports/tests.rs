use super::code_identifiers;
use crate::core::ir::{FunctionDef, TypeRef};
use crate::e2e::codegen::E2eCodegen;
use crate::e2e::codegen::test_support::typed_variant_list_test_support as support;
use crate::e2e::config::{CallConfig, E2eConfig};
use crate::e2e::fixture::{Assertion, Fixture, FixtureGroup};

#[test]
fn type_mentions_in_diagnostics_and_qualified_accesses_do_not_require_imports() {
    let source = r#"
        // Payload Piece
        /* Payload Piece */
        val reason = "Payload Piece"
        val multiline = """Payload Piece"""
        val qualified = sample.Payload.List
        val nested = Parent.Piece
        val actual = result as Actual.Variant
    "#;
    let identifiers = code_identifiers(source, false);
    for unused in ["Payload", "Piece", "List", "Variant"] {
        assert!(!identifiers.contains(unused), "{unused}");
    }
    assert!(identifiers.contains("Actual"));
}

fn fixture(skipped: bool, runnable: bool) -> Fixture {
    let mut assertions = Vec::new();
    if skipped {
        assertions.push(Assertion {
            assertion_type: "contains".into(),
            field: Some("hidden_payload.segments[0].text".into()),
            value: Some(serde_json::json!("Payload Piece")),
            skip: Some(crate::e2e::fixture::AssertionSkip::All(true)),
            ..Default::default()
        });
    }
    if runnable {
        assertions.push(support::assertions(false).remove(0));
    }
    Fixture {
        id: "import_coverage".into(),
        description: "Payload Piece".into(),
        assertions,
        ..Default::default()
    }
}

pub(super) fn generate(skipped: bool, runnable: bool) -> String {
    generate_fixture(fixture(skipped, runnable))
}

fn api() -> crate::core::ir::ApiSurface {
    let mut api = support::api(true, false);
    api.types
        .iter_mut()
        .find(|ty| ty.name == "Envelope")
        .unwrap()
        .fields
        .push(crate::core::ir::FieldDef {
            name: "hidden_payload".into(),
            ty: TypeRef::Named("Payload".into()),
            binding_excluded: true,
            ..Default::default()
        });
    api
}

fn generate_fixture(fixture: Fixture) -> String {
    let api = api();
    let functions = [FunctionDef {
        name: "read_envelope".into(),
        return_type: TypeRef::Named("Envelope".into()),
        ..Default::default()
    }];
    let raw_config: crate::core::config::NewAlefConfig = toml::from_str(
        "[workspace]\nlanguages = ['kotlin_android']\n[[crates]]\nname = 'sample'\nsources = ['src/lib.rs']\n[crates.kotlin_android]\npackage = 'dev.sample'\n",
    ).unwrap();
    let config = raw_config.resolve().unwrap().remove(0);
    let e2e = E2eConfig {
        call: CallConfig {
            function: "read_envelope".into(),
            result_var: "result".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        super::candidates(&[&fixture], &e2e, &api.types, &api.enums, &functions),
        ["Payload", "Piece"],
        "both potential imports must be examined before rendered-code filtering",
    );
    let groups = [FixtureGroup {
        category: "imports".into(),
        fixtures: vec![fixture],
    }];
    crate::e2e::codegen::kotlin_android::KotlinAndroidE2eCodegen
        .generate(&groups, &e2e, &config, &api.types, &api.enums, &functions, &[])
        .unwrap()
        .into_iter()
        .find(|file| file.path.ends_with("ImportsTest.kt"))
        .expect("one full generated Kotlin test file")
        .content
}

#[test]
fn skipped_assertions_do_not_import_union_or_element_types() {
    let source = generate(true, false);
    assert_eq!(
        source
            .lines()
            .filter(|line| line.trim().starts_with("// skipped:"))
            .count(),
        1,
        "{source}"
    );
    for name in ["Payload", "Piece"] {
        assert!(!source.contains(&format!("import dev.sample.{name}\n")), "{source}");
    }
}

#[test]
fn mixed_fixture_imports_only_the_union_used_by_its_runnable_count_assertion() {
    let source = generate(true, true);
    assert_eq!(
        source
            .lines()
            .filter(|line| line.trim().starts_with("// skipped:"))
            .count(),
        1,
        "{source}"
    );
    assert_eq!(source.matches("assertEquals(").count(), 1, "{source}");
    assert!(source.contains("import dev.sample.Payload\n"), "{source}");
    assert!(!source.contains("import dev.sample.Piece\n"), "{source}");
}

#[test]
fn runnable_indexed_assertions_keep_the_element_import() {
    let fixture = Fixture {
        assertions: support::assertions(false),
        ..fixture(false, false)
    };
    let source = generate_fixture(fixture);
    assert_eq!(source.matches("assertEquals(").count(), 3, "{source}");
    for name in ["Payload", "Piece"] {
        assert!(source.contains(&format!("import dev.sample.{name}\n")), "{source}");
    }
}

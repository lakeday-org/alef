use super::*;
use crate::e2e::fixture::{Assertion, Fixture, FixtureGroup};
use serde_json::{Value, json};

struct Case {
    name: &'static str,
    producer: &'static str,
    kind: &'static str,
    expected: Value,
    passes: bool,
}

fn cases() -> Vec<Case> {
    [
        ("max", "max", "equals", json!(u64::MAX), true),
        ("neighbor", "neighbor", "equals", json!(u64::MAX), false),
        ("min", "min", "field_equals", json!(i64::MIN), true),
        ("negative_unsigned", "negative", "equals", json!(u64::MAX), false),
        ("unsigned_negative", "max", "equals", json!(-1), false),
        ("fraction_integer", "fraction", "equals", json!(7), false),
        ("whole_float", "whole", "equals", json!(7), true),
        ("negative_fraction", "negative_fraction", "equals", json!(-7), false),
        ("quarter", "quarter", "equals", json!(0.25), true),
        ("quarter_wrong", "quarter", "equals", json!(0.26), false),
        ("bool_numeric", "truth", "equals", json!(1), false),
        ("one_bool", "one", "equals", json!(true), false),
        ("two_bool", "two", "equals", json!(true), false),
        ("string_numeric", "text", "equals", json!(7), false),
        ("null_numeric", "absent", "equals", json!(0), false),
        ("unsigned_float", "max", "equals", json!(18446744073709551616.0), false),
        (
            "negative_order",
            "negative",
            "greater_than_or_equal",
            json!(u64::MAX),
            false,
        ),
        ("unsigned_order", "max", "greater_than", json!(-1), true),
        ("fraction_order", "fraction", "greater_than", json!(7), true),
        (
            "fraction_order_wrong",
            "fraction",
            "greater_than_or_equal",
            json!(8),
            false,
        ),
        ("huge", "huge", "equals", json!(1e300), true),
        ("huge_order", "huge", "greater_than", json!(1), true),
        ("bool_order", "truth", "greater_than", json!(0), false),
        ("string_order", "text", "greater_than", json!(0), false),
        ("null_order", "absent", "greater_than_or_equal", json!(0), false),
    ]
    .into_iter()
    .map(|(name, producer, kind, expected, passes)| Case {
        name,
        producer,
        kind,
        expected,
        passes,
    })
    .collect()
}

fn suite(api: &ApiSurface, config: &ResolvedCrateConfig, cases: &[Case]) -> String {
    use crate::e2e::codegen::{E2eCodegen, swift::SwiftE2eCodegen};
    let fixtures = cases
        .iter()
        .map(|case| Fixture {
            id: case.name.into(),
            call: Some(case.name.into()),
            assertions: vec![Assertion {
                assertion_type: case.kind.into(),
                field: Some("content.count".into()),
                value: Some(case.expected.clone()),
                ..Default::default()
            }],
            ..Default::default()
        })
        .collect();
    let e2e = crate::e2e::config::E2eConfig {
        calls: cases
            .iter()
            .map(|case| {
                let producer = if case.producer == "absent" {
                    "absent_message".into()
                } else {
                    format!("matrix_{}", case.producer)
                };
                (case.name.into(), super::fixture_call(&producer))
            })
            .collect(),
        ..Default::default()
    };
    SwiftE2eCodegen
        .generate(
            &[FixtureGroup {
                category: "numeric".into(),
                fixtures,
            }],
            &e2e,
            config,
            &api.types,
            &api.enums,
            &api.functions,
            &api.errors,
        )
        .unwrap()
        .into_iter()
        .find(|file| file.path.ends_with("NumericTests.swift"))
        .unwrap()
        .content
}

#[test]
fn full_dispatch_preserves_strict_numeric_and_boolean_domains() {
    let root = tempfile::tempdir().unwrap();
    let (api, config) = super::fixture(root.path());
    let cases = cases();
    assert_eq!(cases.len(), 25);
    let source = suite(&api, &config, &cases);
    assert_eq!(
        source.matches("JSONSerialization.jsonObject").count(),
        cases.len(),
        "{source}"
    );
    assert!(!source.contains("skipped:"), "{source}");
    assert!(!source.contains(".doubleValue"), "{source}");
    assert_eq!(test_methods(&source).len(), cases.len());
}

fn test_methods(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            line.strip_prefix("    func ")
                .and_then(|line| line.split_once('(').map(|(name, _)| name.to_owned()))
        })
        .collect()
}

pub(super) fn verify(root: &Path, api: &ApiSurface, config: &ResolvedCrateConfig) {
    let cases = cases();
    let source = suite(api, config, &cases);
    let methods = test_methods(&source);
    assert_eq!(methods.len(), cases.len());
    super::write(root, "tests/NumericTests.swift", &source);
    let output = super::native_task(root, "suite", "", false);
    super::write(
        root,
        "numeric-matrix.log",
        &format!("{}\n{}", output.stdout, output.stderr),
    );
    assert!(!output.success, "negative numeric controls all passed");
    for (case, name) in cases.iter().zip(methods) {
        let marker = format!("{name}]'");
        let results: Vec<_> = output
            .stdout
            .lines()
            .filter(|line| {
                line.contains("Test Case")
                    && line.contains(&marker)
                    && (line.contains("passed") || line.contains("failed"))
            })
            .collect();
        assert_eq!(
            results.len(),
            1,
            "case {} missing runtime verdict: {}\n{}",
            case.name,
            output.stdout,
            output.stderr
        );
        assert_eq!(
            results[0].contains("passed"),
            case.passes,
            "{}: {}",
            case.name,
            output.stdout
        );
    }
    println!(
        "structured_numeric_matrix_cases={} positive={} negative={}",
        cases.len(),
        cases.iter().filter(|case| case.passes).count(),
        cases.iter().filter(|case| !case.passes).count()
    );
}

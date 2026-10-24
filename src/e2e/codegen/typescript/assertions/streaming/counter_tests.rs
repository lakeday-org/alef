use super::*;

#[test]
fn usage_counters_keep_exact_integer_and_fractional_expectations() {
    for (value, numerator, denominator) in [
        (serde_json::json!(9007199254740993u64), "9007199254740993", "1"),
        (serde_json::json!(u64::MAX), "18446744073709551615", "1"),
        (serde_json::json!(-2.5), "-25", "10"),
        (serde_json::json!(1e20), "100000000000000000000", "1"),
    ] {
        let assertion = Assertion {
            assertion_type: "equals".into(),
            field: Some("usage.total_tokens".into()),
            value: Some(value),
            ..Default::default()
        };
        let mut source = String::new();
        assert!(render_exact_counter(
            &mut source,
            &assertion,
            "usage.total_tokens",
            "counter"
        ));
        assert!(source.contains(&format!("BigInt(\"{numerator}\")")), "{source}");
        assert!(source.contains(&format!("BigInt(\"{denominator}\")")), "{source}");
        assert!(!source.contains("Number(counter)"), "{source}");
    }
}

#[test]
#[ignore = "requires Node.js"]
fn exact_usage_assertions_execute_without_bigint_rounding_or_float_regressions() {
    use crate::e2e::codegen::streaming_assertions::probe_support::{Probe, assert_status};
    let probe = Probe::new("node");
    assert_eq!(NATIVE_CASES.len(), 11);
    for (actual, value, kind, success) in NATIVE_CASES.iter().cloned() {
        let assertion = Assertion {
            assertion_type: kind.into(),
            field: Some("usage.total_tokens".into()),
            value: Some(serde_json::from_str(value).expect("numeric fixture")),
            ..Default::default()
        };
        let mut body = String::new();
        assert!(render_exact_counter(
            &mut body,
            &assertion,
            "usage.total_tokens",
            "counter"
        ));
        let source = crate::e2e::template_env::render(
            "typescript/usage_runtime_probe.jinja",
            crate::alef_context! {actual => actual,assertion => body},
        );
        assert_status(&probe.run(&source), success);
    }
    let old = crate::e2e::template_env::render(
        "typescript/usage_runtime_probe.jinja",
        crate::alef_context! {
            actual => "9007199254740993n", assertion => "expect(Number(counter)).toBe(9007199254740992);",
        },
    );
    assert_status(&probe.run(&old), true);
}

type NativeCase = (&'static str, &'static str, &'static str, bool);
const NATIVE_CASES: &[NativeCase] = &[
    ("9007199254740993n", "9007199254740993", "equals", true),
    ("9007199254740993n", "9007199254740992", "equals", false),
    ("18446744073709551615n", "18446744073709551615", "equals", true),
    ("18446744073709551615n", "18446744073709551614", "equals", false),
    ("-3n", "-3", "equals", true),
    ("0.25", "0.25", "equals", true),
    ("0.25", "0.26", "equals", false),
    ("1e20", "1e20", "equals", true),
    ("7n", "6.5", "greater_than", true),
    ("null", "0", "equals", false),
    ("NaN", "0", "equals", false),
];

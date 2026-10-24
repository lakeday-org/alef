use super::*;

#[test]
fn numeric_counter_reads_are_never_narrowed_to_go_int() {
    let assertion = Assertion {
        assertion_type: "equals".into(),
        field: Some("usage.total_tokens".into()),
        value: Some(serde_json::json!(u64::MAX)),
        ..Default::default()
    };
    let mut source = String::new();
    assert!(render_exact_counter(
        &mut source,
        &assertion,
        "usage.total_tokens",
        "counter"
    ));
    assert!(source.contains("big.Rat"), "{source}");
    assert!(
        !regex::Regex::new(r"\bint\(counter\)")
            .expect("cast pattern")
            .is_match(&source),
        "{source}"
    );
    assert!(source.contains("18446744073709551615"), "{source}");
}

#[test]
#[ignore = "requires Go"]
fn native_go_counters_preserve_unsigned_signed_and_fractional_values() {
    use crate::e2e::codegen::streaming_assertions::probe_support::{Probe, assert_status};
    let probe = Probe::new("go");
    assert_eq!(NATIVE_CASES.len(), 13);
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
            "go/usage_runtime_probe.jinja",
            crate::alef_context! {actual => actual,assertion => body},
        );
        assert_status(&probe.run(&source), success);
    }
    let old = crate::e2e::template_env::render(
        "go/usage_runtime_probe.jinja",
        crate::alef_context! {
            actual => "uint64(18446744073709551615)", assertion => "assert.True(t, int(counter) >= 10)",
        },
    );
    assert_status(&probe.run(&old), false);
}

type NativeCase = (&'static str, &'static str, &'static str, bool);
const NATIVE_CASES: &[NativeCase] = &[
    ("numericAlias(7)", "7", "equals", true),
    ("float32(0.1)", "0.1", "equals", true),
    ("\"18\"", "18", "equals", false),
    ("uint64(18446744073709551615)", "18446744073709551615", "equals", true),
    ("uint64(18446744073709551615)", "10", "greater_than_or_equal", true),
    ("uint64(18446744073709551615)", "18446744073709551614", "equals", false),
    ("int64(-1)", "18446744073709551615", "equals", false),
    ("int64(-2)", "-3", "greater_than_or_equal", true),
    ("float64(0.25)", "0.25", "equals", true),
    ("float64(0.25)", "0.26", "equals", false),
    ("float64(1e20)", "1e20", "equals", true),
    ("uint32(7)", "6.5", "greater_than", true),
    ("(*uint64)(nil)", "0", "equals", false),
];

use super::*;

fn surface(primitive: PrimitiveType) -> Vec<TypeDef> {
    vec![
        TypeDef {
            name: "Chunk".into(),
            fields: vec![crate::core::ir::FieldDef {
                name: "usage".into(),
                ty: TypeRef::Optional(Box::new(TypeRef::Named("Usage".into()))),
                ..Default::default()
            }],
            ..Default::default()
        },
        TypeDef {
            name: "Usage".into(),
            fields: vec![crate::core::ir::FieldDef {
                name: "total_tokens".into(),
                ty: TypeRef::Primitive(primitive),
                ..Default::default()
            }],
            ..Default::default()
        },
    ]
}

fn fixture(value: serde_json::Value, kind: &str) -> Fixture {
    serde_json::from_value(serde_json::json!({"id":"counter", "input":null,
        "assertions":[{"type":kind,"field":"usage.total_tokens","value":value}]}))
    .expect("fixture")
}

#[test]
fn integer_storage_tracks_ir_width_and_sign_without_casts() {
    for (primitive, expected) in [
        (PrimitiveType::U64, "uint64_t"),
        (PrimitiveType::Usize, "uintptr_t"),
        (PrimitiveType::I64, "int64_t"),
        (PrimitiveType::I8, "int8_t"),
    ] {
        let fixture = fixture(serde_json::json!(10), "greater_than_or_equal");
        let probe = UsageProbe::resolve(&fixture, &surface(primitive), "Chunk");
        let mut out = String::new();
        probe.render_declarations(&mut out);
        probe.render_capture(&mut out, "sample", "chunk", "chunk", "Sample");
        assert!(out.contains(expected), "{out}");
        assert!(!out.contains("long long"), "{out}");
    }
}

#[test]
fn skipped_usage_types_and_fields_never_emit_getters() {
    for excluded in ["chunk", "usage", "counter", "usage_type"] {
        let fixture = fixture(serde_json::json!(10), "equals");
        let mut types = surface(PrimitiveType::U64);
        match excluded {
            "chunk" => types[0].binding_excluded = true,
            "usage" => types[0].fields[0].binding_excluded = true,
            "counter" => types[1].fields[0].binding_excluded = true,
            _ => types[1].binding_excluded = true,
        }
        let probe = UsageProbe::resolve(&fixture, &types, "Chunk");
        assert!(!probe.has_fields(), "{excluded}");
        let mut source = String::new();
        probe.render_capture(&mut source, "sample", "chunk", "chunk", "Sample");
        assert!(source.is_empty(), "{source}");
    }
}

#[test]
fn signed_and_unsigned_extremes_render_exact_constants() {
    for (signed, value, expected) in [
        (false, serde_json::json!(u64::MAX), "18446744073709551615ULL"),
        (true, serde_json::json!(i64::MIN), "(-9223372036854775807LL - 1)"),
        (false, serde_json::json!(-1), ""),
    ] {
        let number = value.as_number().expect("number");
        let (literal, constant) = numeric_expectation(number, signed, ">=").expect("integer");
        assert_eq!(literal, expected);
        assert_eq!(constant, (!signed && value == serde_json::json!(-1)).then_some(true));
    }
}

#[test]
fn wire_renames_do_not_change_the_ffi_getter_or_local() {
    let mut types = surface(PrimitiveType::U64);
    types[1].fields[0].serde_rename = Some("token-count".into());
    let mut fixture = fixture(serde_json::json!(10), "equals");
    fixture.assertions[0].field = Some("usage.token-count".into());
    let probe = UsageProbe::resolve(&fixture, &types, "Chunk");
    let mut source = String::new();
    probe.render_capture(&mut source, "sample", "chunk", "chunk", "Sample");
    assert!(source.contains("sample_usage_total_tokens(chunk_usage)"), "{source}");
    assert!(source.contains("last_usage_total_tokens"), "{source}");
    assert!(!source.contains("token-count"), "{source}");
    assert!(probe.render_assertion(&mut source, &fixture.assertions[0]));
}

#[test]
#[ignore = "requires a C compiler"]
fn native_c_usage_preserves_width_cross_sign_and_last_chunk_presence() {
    use crate::e2e::codegen::streaming_assertions::probe_support::{Probe, assert_status};
    let native = Probe::new("c");
    assert_eq!(NATIVE_CASES.len(), 9);
    for (primitive, actual, value, kind, absent, success) in NATIVE_CASES.iter().cloned() {
        let fixture = fixture(serde_json::from_str(value).expect("numeric fixture"), kind);
        let usage = UsageProbe::resolve(&fixture, &surface(primitive), "Chunk");
        let (declarations, capture, assertion) = render(&usage, &fixture);
        let source = crate::e2e::template_env::render(
            "c/usage_runtime_probe.jinja",
            crate::alef_context! {
                declarations => declarations,capture => capture,assertion => assertion,actual => actual,
                c_type => usage.fields[0].c_type,absent => absent,
            },
        );
        assert_status(&native.run(&source), success);
    }
    let fixture = fixture(serde_json::json!(10), "greater_than_or_equal");
    let usage = UsageProbe::resolve(&fixture, &surface(PrimitiveType::U64), "Chunk");
    let (declarations, capture, assertion) = render(&usage, &fixture);
    let old = crate::e2e::template_env::render(
        "c/usage_runtime_probe.jinja",
        crate::alef_context! {
            declarations => declarations.replace("uint64_t", "long long"),
            capture => capture.replace("= sample_usage_total_tokens", "= (long long)sample_usage_total_tokens"),
            assertion => assertion.replace("10ULL", "10"), actual => "UINT64_MAX", c_type => "uint64_t", absent => false,
        },
    );
    assert_status(&native.run(&old), false);
}

fn render(usage: &UsageProbe, fixture: &Fixture) -> (String, String, String) {
    let (mut declarations, mut capture, mut assertion) = (String::new(), String::new(), String::new());
    usage.render_declarations(&mut declarations);
    usage.render_capture(&mut capture, "sample", "chunk", "chunk", "Sample");
    assert!(usage.render_assertion(&mut assertion, &fixture.assertions[0]));
    (declarations, capture, assertion)
}

type NativeCase = (PrimitiveType, &'static str, &'static str, &'static str, bool, bool);
const NATIVE_CASES: &[NativeCase] = &[
    (
        PrimitiveType::U64,
        "UINT64_MAX",
        "10",
        "greater_than_or_equal",
        false,
        true,
    ),
    (
        PrimitiveType::Usize,
        "UINTPTR_MAX",
        "10",
        "greater_than_or_equal",
        false,
        true,
    ),
    (
        PrimitiveType::U64,
        "UINT64_MAX",
        "18446744073709551615",
        "equals",
        false,
        true,
    ),
    (
        PrimitiveType::U64,
        "UINT64_MAX",
        "18446744073709551614",
        "equals",
        false,
        false,
    ),
    (PrimitiveType::I64, "-1", "18446744073709551615", "equals", false, false),
    (
        PrimitiveType::I64,
        "-1",
        "18446744073709551615",
        "greater_than_or_equal",
        false,
        false,
    ),
    (PrimitiveType::I64, "-2", "-3", "greater_than_or_equal", false, true),
    (PrimitiveType::U64, "5", "-1", "greater_than", false, true),
    (PrimitiveType::U64, "0", "0", "equals", true, false),
];

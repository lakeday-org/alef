use super::*;
use crate::core::ir::{ErrorDef, ErrorVariant, FieldDef, TypeRef};

fn variant(template: &str, fields: &[&str]) -> ErrorVariant {
    ErrorVariant {
        name: "Rejected".into(),
        message_template: Some(template.into()),
        fields: fields
            .iter()
            .map(|name| FieldDef {
                name: (*name).into(),
                ty: TypeRef::String,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

fn metadata_for(variant: ErrorVariant) -> String {
    let error = ErrorDef {
        name: "SampleError".into(),
        variants: vec![variant],
        exception_base: None,
        rust_path: "sample::SampleError".into(),
        original_rust_path: String::new(),
        doc: String::new(),
        methods: Vec::new(),
        binding_excluded: false,
        binding_exclusion_reason: None,
        version: Default::default(),
    };
    metadata_comments(&[&error])
}

#[test]
fn metadata_uses_only_visible_scalar_fields_and_retains_unsupported_fallbacks() {
    assert!(!metadata_for(variant("rejected: {raw_message}", &["raw_message"])).is_empty());
    for template in ["{missing}", "{raw_message:?}", "{raw_message:>8}", "{", "}"] {
        assert!(metadata_for(variant(template, &["raw_message"])).is_empty());
    }
    for kind in ["hidden", "optional", "sanitized", "json"] {
        let mut case = variant("{message}", &["message"]);
        match kind {
            "hidden" => case.fields[0].binding_excluded = true,
            "optional" => case.fields[0].optional = true,
            "sanitized" => case.fields[0].sanitized = true,
            _ => case.fields[0].ty = TypeRef::Json,
        }
        assert!(metadata_for(case).is_empty(), "{kind}");
    }
    let mut tuple = variant("{1}", &["_0", "_1"]);
    tuple.is_tuple = true;
    tuple.fields[0].binding_excluded = true;
    assert!(metadata_for(tuple.clone()).contains("\"field\":0"));
    tuple.message_template = Some("{0}".into());
    assert!(metadata_for(tuple).is_empty());
    let unit = ErrorVariant {
        is_unit: true,
        ..variant("literal", &[])
    };
    assert!(
        metadata_for(unit).is_empty(),
        "all-unit plain Dart enums remain unchanged"
    );
}

const BRIDGE: &str =
    "sealed class SampleError {\n const factory SampleError.rejected({required String renamedLabel}) = ActualImpl;\n}";
const FREEZED: &str = "class ActualImpl extends SampleError {\n const ActualImpl({required this.renamedLabel});\n final String renamedLabel;\n @override\n String toString() {\n return 'SampleError.rejected(renamedLabel: $renamedLabel)';\n }\n}\n";

#[test]
fn actual_factory_aliases_and_bound_fields_are_rewritten_idempotently() {
    let metadata = metadata_for(variant("prefix {{ {raw_message} }} $literal", &["raw_message"]));
    let result = rewrite(FREEZED, BRIDGE, &metadata).unwrap();
    assert!(result.contains("renamedLabel.toString()"), "{result}");
    assert!(result.contains("\\$literal"), "{result}");
    assert_eq!(rewrite(&result, BRIDGE, &metadata).unwrap(), result);
    assert!(!result.contains("SampleError.rejected(renamedLabel:"));
    for (source, bridge) in [
        ("".to_string(), BRIDGE.to_string()),
        (FREEZED.repeat(2), BRIDGE.to_string()),
        (FREEZED.to_string(), BRIDGE.repeat(2)),
        (FREEZED.replace("toString", "otherMethod"), BRIDGE.to_string()),
        (
            FREEZED.replace("this.renamedLabel", "this.renamedLabel, required this.extra"),
            BRIDGE.to_string(),
        ),
    ] {
        assert!(
            rewrite(&source, &bridge, &metadata).is_err(),
            "missing/duplicate layout must reject"
        );
    }
    assert!(rewrite(FREEZED, BRIDGE, &(metadata.clone() + "\n" + &metadata)).is_err());
    assert_eq!(rewrite(FREEZED, BRIDGE, "unrelated mirror").unwrap(), FREEZED);
}

#[test]
fn lexical_bounds_ignore_strings_nested_interpolations_comments_and_generic_commas() {
    let source = r##"{ /* class Fake { /* nested */ } */ var x = r'\'; var y = '''}'''; var z = '${f("}")}'; }"##;
    let masked = lexical::mask(source).unwrap();
    assert_eq!(masked.len(), source.len());
    assert!(!masked.contains("class Fake"));
    assert_eq!(lexical::closing(source, 0).unwrap(), source.len() - 1);
    assert_eq!(
        lexical::parameters("{required Map<String, List<int>> values, required this.label}").unwrap(),
        ["values", "label"]
    );
    for malformed in ["{", "{]", "'unclosed", "/* unclosed"] {
        assert!(
            lexical::mask(malformed)
                .and_then(|_| lexical::closing(malformed, 0))
                .is_err()
        );
    }
}

#[path = "native.rs"]
mod native;

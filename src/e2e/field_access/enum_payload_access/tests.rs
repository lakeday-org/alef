use super::*;
use std::collections::{HashMap, HashSet};

#[path = "tests/core_fixture.rs"]
mod core_fixture;

fn resolver() -> FieldResolver {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("lib.rs");
    std::fs::write(&source, include_str!("tests/core_fixture.rs")).unwrap();
    let api = crate::extract::extractor::extract(&[source.as_path()], "sample_core", "1.0.0", None).unwrap();
    assert_eq!(api.enums.len(), 8);
    FieldResolver::new(
        &HashMap::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
    )
    .with_ir_enum_map(
        FieldResolver::ir_enum_fields(&api.types, &api.enums),
        Some("Payload".into()),
    )
}

#[test]
fn enum_payload_metadata_retains_original_fields_and_canonical_wire_keys() {
    let resolver = resolver();
    let definition = resolver.enum_definition("Piece").unwrap();
    assert_eq!(definition.variants.len(), 2);
    for spelling in ["text", "value"] {
        let field = resolver.enum_field_metadata("Piece", spelling).unwrap();
        assert_eq!(
            (&*field.variant, &*field.field, &*field.wire_key),
            ("Text", "text", "value")
        );
        assert_eq!(field.definition.name, "text");
        assert_eq!(field.definition.ty, crate::core::ir::TypeRef::String);
        assert_eq!(field.definition.serde_rename.as_deref(), Some("value"));
        assert_eq!(field.kind, WireValueKind::String);
        assert!(field.next_type.is_none());
        assert!(field.wire_prefix.is_empty());
    }
    assert_eq!(
        resolver.enum_field_metadata("Piece", "note_text").unwrap().wire_key,
        "noteText"
    );
    assert_eq!(
        resolver.enum_field_metadata("Piece", "noteText").unwrap().field,
        "note_text"
    );
    assert!(resolver.enum_field_metadata("AmbiguousPiece", "shared").is_none());
    assert!(resolver.enum_field_metadata("Piece", "unknown").is_none());
    assert!(resolver.enum_definition("Unknown").is_none());
}

#[test]
fn enum_payload_wire_steps_follow_serialized_list_values_and_reject_the_old_wire_key() {
    let resolver = resolver();
    let value = core_fixture::Payload::Segments(vec![
        core_fixture::Piece::Text { text: "hello".into() },
        core_fixture::Piece::Text { text: " world".into() },
    ]);
    let wire = serde_json::to_value(value).unwrap();
    assert_eq!(wire.as_array().unwrap().len(), 2);
    for (suffix, expected) in [("[0].text", "hello"), ("[1].value", " world")] {
        let steps = resolver.enum_payload_wire_steps("Payload", "Segments", suffix).unwrap();
        assert_eq!(
            read_wire(&wire, &steps).and_then(serde_json::Value::as_str),
            Some(expected)
        );
    }
    let old = [WireAccessStep::Index(0), WireAccessStep::Field("text".into())];
    assert_eq!(
        read_wire(&wire, &old),
        None,
        "raw host field must not pass as a wire key"
    );
    assert_eq!(
        resolver.enum_payload_wire_steps("Payload", "Segments", ""),
        Some(Vec::new())
    );
    let note = serde_json::to_value(core_fixture::Payload::Segments(vec![core_fixture::Piece::Note {
        note_text: "note".into(),
    }]))
    .unwrap();
    let steps = resolver
        .enum_payload_wire_steps("Payload", "Segments", "[0].note_text")
        .unwrap();
    assert_eq!(
        read_wire(&note, &steps).and_then(serde_json::Value::as_str),
        Some("note")
    );
    assert!(
        serde_json::to_value(core_fixture::Payload::Text("plain".into()))
            .unwrap()
            .is_string()
    );
}

fn read_wire<'a>(mut value: &'a serde_json::Value, steps: &[WireAccessStep]) -> Option<&'a serde_json::Value> {
    for step in steps {
        value = match step {
            WireAccessStep::Field(field) => value.get(field)?,
            WireAccessStep::Index(index) => value.get(*index)?,
        };
    }
    Some(value)
}

#[test]
fn enum_payload_wire_prefixes_and_refusals_match_the_declared_representation() {
    let resolver = resolver();
    for (owner, prefix) in [
        ("Payload", Vec::new()),
        ("ExternalPayload", vec!["segmentsWire".to_string()]),
        ("AdjacentPayload", vec!["body".to_string()]),
    ] {
        assert_eq!(resolver.enum_payload_wire_prefix(owner, "Segments"), Some(prefix));
    }
    assert_eq!(
        resolver.enum_payload_wire_steps("ExternalPieces", "Segments", "[0].text"),
        Some(vec![
            WireAccessStep::Index(0),
            WireAccessStep::Field("Text".into()),
            WireAccessStep::Field("text".into()),
        ])
    );
    for suffix in ["[wrong].text", "[-1].text", "[0]..text", "[0].unknown", "[0].text[0]"] {
        assert_eq!(
            resolver.enum_payload_wire_steps("Payload", "Segments", suffix),
            None,
            "{suffix}"
        );
    }
    assert_eq!(
        resolver.enum_payload_wire_steps("AmbiguousPayload", "Segments", "[0].shared"),
        None
    );
    assert_eq!(resolver.enum_payload_wire_steps("Missing", "Segments", ""), None);
    assert_eq!(resolver.enum_payload_wire_steps("Payload", "Missing", "[0].text"), None);
    check_wrapped_values(&resolver);
}

fn check_wrapped_values(resolver: &FieldResolver) {
    let external = serde_json::to_value(core_fixture::ExternalPayload::Segments(Vec::new())).unwrap();
    assert!(external.get("segmentsWire").unwrap().as_array().unwrap().is_empty());
    let adjacent = serde_json::to_value(core_fixture::AdjacentPayload::Segments(Vec::new())).unwrap();
    assert!(adjacent.get("body").unwrap().as_array().unwrap().is_empty());
    let piece = core_fixture::ExternalPiece::Text { text: "wrapped".into() };
    let value = serde_json::to_value(core_fixture::ExternalPieces::Segments(vec![piece])).unwrap();
    let array = value.get("Segments").unwrap();
    let steps = resolver
        .enum_payload_wire_steps("ExternalPieces", "Segments", "[0].text")
        .unwrap();
    assert_eq!(
        read_wire(array, &steps).and_then(serde_json::Value::as_str),
        Some("wrapped")
    );
    let ambiguous = core_fixture::AmbiguousPayload::Segments(vec![
        core_fixture::AmbiguousPiece::First { shared: "a".into() },
        core_fixture::AmbiguousPiece::Second { shared: "b".into() },
    ]);
    assert_eq!(
        serde_json::to_value(ambiguous).unwrap()["Segments"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

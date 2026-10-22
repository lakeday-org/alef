use super::*;

fn metadata_enum() -> EnumDef {
    let mut boxed = variant(
        "Boxed",
        vec![{
            let mut f = field("_0", TypeRef::Named("BoxedMetadata".to_string()));
            f.is_boxed = true;
            f
        }],
    );
    boxed.cfg = Some(r#"feature = "office""#.to_string());

    enum_def(
        "SampleMetadata",
        vec![
            variant("Basic", vec![field("_0", TypeRef::Named("BasicMetadata".to_string()))]),
            boxed,
            variant(
                "MultiPart",
                vec![field("_0", TypeRef::Named("MultiPartMetadata".to_string()))],
            ),
            variant("Empty", vec![]),
            EnumVariant {
                is_tuple: true,
                ..variant("Pair", vec![field("_0", TypeRef::String), field("_1", TypeRef::String)])
            },
        ],
    )
}

/// The stub must declare a bare-annotation property for every variant `write_pyo3_variant_accessors`
/// gives a typed getter, while reserving sibling factory names. ~keep
#[test]
fn should_declare_a_stub_property_for_every_runtime_variant_getter() {
    let stub = gen_enum_stub(&metadata_enum(), false, &no_dtos(), true, &[]);

    assert!(stub.contains("    basic_payload: BasicMetadata | None"), "{stub}");
    assert!(stub.contains("def basic(_0: BasicMetadata)"), "{stub}");
    assert!(!stub.contains("    basic: BasicMetadata | None"), "{stub}");
    assert!(
        stub.contains("    boxed_payload: BoxedMetadata | None"),
        "boxed field must still resolve to the unboxed inner type: {stub}"
    );
    assert!(
        stub.contains("    multi_part_payload: MultiPartMetadata | None"),
        "multi-word PascalCase variant name must snake_case correctly: {stub}"
    );
}

/// A unit variant and a multi-field tuple variant have no typed `#[getter]`, but the runtime
/// still gives each an untyped dict-shaped one (GH#581), so the stub declares it as `Any | None`
/// rather than a typed property. ~keep
#[test]
fn should_declare_an_untyped_property_for_a_variant_with_a_dict_shaped_getter() {
    let stub = gen_enum_stub(&metadata_enum(), false, &no_dtos(), true, &[]);

    assert!(stub.contains("    empty: Any | None"), "unit variant: {stub}");
    assert!(
        stub.contains("    pair: Any | None"),
        "multi-field tuple variant: {stub}"
    );
}

/// `write_pyo3_variant_accessors` emits the `#[getter]` for a foreign-cfg-gated variant
/// unconditionally — only the match ARM inside it is dropped when `is_host_enum` is false, never
/// the getter method itself — so the stub property must stay declared regardless of
/// `is_host_enum`, unlike the variant constructors which really do disappear. ~keep
#[test]
fn should_keep_the_stub_property_for_a_foreign_cfg_gated_variant_even_when_unreachable() {
    let stub = gen_enum_stub(&metadata_enum(), false, &no_dtos(), false, &[]);

    assert!(
        stub.contains("    boxed_payload: BoxedMetadata | None"),
        "the getter method exists in every build; only its match arm is cfg-conditional: {stub}"
    );
}

/// A genuine multi-field tuple variant keeps its declared positional shape -- only the
/// single-field case (which never has a `_0` key on any wire form) is suppressed. ~keep
#[test]
fn multi_field_tuple_variant_keeps_its_positional_fields() {
    let stub = gen_enum_stub(&metadata_enum(), false, &no_dtos(), true, &[]);

    assert!(stub.contains("class SampleMetadataPairVariant(TypedDict):"), "{stub}");
    assert!(stub.contains("    _0: str"), "{stub}");
    assert!(stub.contains("    _1: str"), "{stub}");
}

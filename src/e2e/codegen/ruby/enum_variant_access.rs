//! Ruby tagged-enum assertion access, matching Magnus's native payload and Hash lowerings.
//!
//! Native tagged newtypes expose a variant predicate and `value` object. Other data variants
//! retain serde Hash navigation; both paths require IR proof of the variant and payload field.

use std::collections::HashSet;

use heck::ToUpperCamelCase;

use crate::core::ir::EnumDef;
use crate::e2e::field_access::FieldResolver;

/// Data-carrying enums requiring payload-aware assertions rather than Symbol comparisons.
/// Native typed variants are distinguished separately by the binding's shared lowering predicate.
pub(super) fn hash_serialized_enum_names(enums: &[EnumDef]) -> HashSet<String> {
    enums
        .iter()
        .filter(|enum_def| enum_def.variants.iter().any(|variant| !variant.fields.is_empty()))
        .map(|enum_def| enum_def.name.clone())
        .collect()
}

/// What the Ruby binding offers for a fixture field path that may cross a hash-serialized enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RubyEnumAccess {
    /// Nothing to refuse: the path crosses no enum, or every enum on it is a `Symbol`.
    Available,
    /// The path lands on a data enum whose aggregate representation cannot be compared as a scalar.
    SerializedAsHash,
    /// The path crosses a data enum and requires representation-specific payload navigation.
    VariantAccessorUnavailable,
}

/// Classify `field` by walking each of its leading segments and asking the IR (via
/// `field_resolver.ruby_enum_serialized_as_hash`) whether that prefix resolves to a
/// hash-serialized enum. Entirely name-agnostic: it fires for any field path the crate's own IR
/// confirms crosses a hash-serialized enum, under whatever name that field happens to have, and
/// it does NOT fire for a field that merely shares a literal name with one but resolves to
/// something else (a plain scalar, a unit-variant/`Symbol` enum, or nothing the IR recognizes).
pub(super) fn classify(field_resolver: &FieldResolver, field: &str) -> RubyEnumAccess {
    if field_resolver.ruby_enum_serialized_as_hash(field) == Some(true) {
        return RubyEnumAccess::SerializedAsHash;
    }
    let segments: Vec<&str> = field.split('.').collect();
    for i in 1..segments.len() {
        let prefix = segments[..i].join(".");
        if field_resolver.ruby_enum_serialized_as_hash(&prefix) == Some(true) {
            return RubyEnumAccess::VariantAccessorUnavailable;
        }
    }
    RubyEnumAccess::Available
}

/// Render the one Ruby-accessible tagged-enum payload shape Alef can prove from the IR: a path
/// crosses a known single-payload variant and names one plain field on that payload.
///
/// Native typed payloads use the binding's predicate and `value` accessor. For remaining Hash
/// payloads, how many levels sit between the discriminator and the payload field depends on what
/// `backends::magnus` emitted, and there are three cases.
///
/// `enum_magnus.rs.jinja` restates every single-payload variant as a Rust STRUCT variant
/// (`Excel { _0: ExcelMetadata }`). For an INTERNALLY tagged enum whose variant is a tuple variant
/// it also puts `#[serde(flatten)]` on that field (`gen_enum`'s `flatten_newtype`), so
/// `serde_json::to_value` produces core's own flat wire —
/// `{"format_type":"excel","sheet_count":2,...}` — and there is NO hop: the payload field sits
/// beside the discriminator. That is the common case and the one this generator got wrong.
///
/// Without the flatten the payload nests under the field's own name (`"_0"`), and an
/// ADJACENTLY tagged enum (`#[serde(tag = "..", content = "..")]`) nests under its configured
/// `content` key regardless of tuple/struct form, because serde puts adjacently-tagged payloads
/// there unconditionally. Both of those need the hop.
///
/// Emitting the wrong one of the three is a `KeyError` at runtime, in either direction — alef
/// 0.85.11 taught the binding to flatten and left this generator emitting the `_0` hop, and a
/// consumer's Ruby suite was red from its next release onward. `union_variant_payload_is_tuple` and
/// `union_variant_payload` read the same IR the magnus backend reads, so the two cannot drift
/// apart again without the flag itself changing meaning.
///
/// ~keep Collection suffixes use wire metadata; other unsupported suffixes retain the explicit generator-gap skip.
pub(super) fn variant_field_accessor(field_resolver: &FieldResolver, field: &str, result_var: &str) -> Option<String> {
    if let Some(accessor) = super::enum_collection_access::accessor(field_resolver, field, result_var) {
        return Some(accessor);
    }
    let resolved = field_resolver.resolve(field);
    let segments: Vec<&str> = resolved.split('.').collect();
    for enum_index in 1..segments.len().saturating_sub(1) {
        let prefix = segments[..enum_index].join(".");
        if field_resolver.ruby_enum_serialized_as_hash(&prefix) != Some(true) {
            continue;
        }

        let variant = segments[enum_index];
        let [payload_field] = segments.get(enum_index + 1..)? else {
            return None;
        };
        if !payload_field
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
        {
            return None;
        }
        let enum_type = field_resolver.ir_enum_type_name(&prefix)?;
        let variant = variant.to_upper_camel_case();
        let (wrapped_field, payload_type) = field_resolver.union_variant_payload(&enum_type, &variant)?;
        if field_resolver.is_declared_field_of_type(payload_type, payload_field) != Some(true) {
            return None;
        }
        if field_resolver.ruby_variant_has_typed_payload(&enum_type, &variant) {
            let predicate = crate::codegen::naming::pascal_to_snake(&variant);
            let enum_value = field_resolver.accessor(&prefix, "ruby", result_var);
            return Some(format!(
                concat!(
                    "{enum_value}.then {{ |enum_value| raise \"unexpected tagged enum variant\" ",
                    "unless enum_value.{predicate}?; enum_value.value.{payload_field} }}"
                ),
                enum_value = enum_value,
                predicate = predicate,
                payload_field = payload_field,
            ));
        }
        let (serde_tag, wire_variant) = field_resolver.tagged_enum_wire_discriminator(&enum_type, &variant)?;
        let content_key = field_resolver.tagged_enum_content_key(&enum_type);
        let flattened = content_key.is_none() && field_resolver.union_variant_payload_is_tuple(&enum_type, &variant);
        let magnus_wrapper_key = content_key.unwrap_or(wrapped_field);

        let enum_hash = field_resolver.accessor(&prefix, "ruby", result_var);
        let tag = crate::e2e::escape::ruby_string_literal(serde_tag);
        let wire_variant = crate::e2e::escape::ruby_string_literal(wire_variant);
        let magnus_wrapper_key = crate::e2e::escape::ruby_string_literal(magnus_wrapper_key);
        let payload_field = crate::e2e::escape::ruby_string_literal(payload_field);
        let payload_access = if flattened {
            format!("enum_hash.fetch({payload_field}.to_sym)")
        } else {
            format!("enum_hash.fetch({magnus_wrapper_key}.to_sym).fetch({payload_field}.to_sym)")
        };
        return Some(format!(
            concat!(
                "{enum_hash}.then {{ |enum_hash| raise \"unexpected tagged enum variant\" ",
                "unless enum_hash.fetch({tag}.to_sym) == {wire_variant}; ",
                "{payload_access} }}"
            ),
            enum_hash = enum_hash,
            tag = tag,
            wire_variant = wire_variant,
            payload_access = payload_access,
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{RubyEnumAccess, classify, hash_serialized_enum_names};
    use crate::core::ir::{EnumDef, EnumVariant, FieldDef, PrimitiveType, TypeDef, TypeRef};
    use crate::e2e::field_access::FieldResolver;

    fn field(name: &str, ty: TypeRef) -> FieldDef {
        FieldDef {
            name: name.to_string(),
            ty,
            ..FieldDef::default()
        }
    }

    fn named(name: &str) -> TypeRef {
        TypeRef::Named(name.to_string())
    }

    /// The fixture covers native tagged newtypes, Hash payloads and unit/Symbol variants.
    ///
    /// `metadata` deliberately carries a plain `String` field named `format` — the exact literal
    /// spelling the old guard matched on — so a test can prove that name alone no longer triggers
    /// the skip.
    fn ir() -> (Vec<TypeDef>, Vec<EnumDef>) {
        let type_defs = vec![
            TypeDef {
                name: "ProcessingResult".to_string(),
                fields: vec![
                    field("summary", named("DocumentSummary")),
                    field("metadata", named("PageMetadata")),
                ],
                ..TypeDef::default()
            },
            TypeDef {
                name: "DocumentSummary".to_string(),
                fields: vec![
                    field("encoding", named("EncodingDetails")),
                    field("kind", named("DocumentKind")),
                    field("adjacent", named("AdjacentDetails")),
                ],
                ..TypeDef::default()
            },
            TypeDef {
                name: "PageMetadata".to_string(),
                fields: vec![field("format", TypeRef::String)],
                ..TypeDef::default()
            },
            TypeDef {
                name: "SpreadsheetDetails".to_string(),
                fields: vec![field("sheet_count", TypeRef::Primitive(PrimitiveType::U32))],
                ..TypeDef::default()
            },
            TypeDef {
                name: "WrappedPayload".to_string(),
                fields: vec![field("value", TypeRef::String)],
                ..TypeDef::default()
            },
        ];
        let enums = vec![
            EnumDef {
                name: "EncodingDetails".to_string(),
                serde_tag: Some("type'kind".to_string()),
                variants: vec![
                    EnumVariant {
                        name: "Spreadsheet".to_string(),
                        serde_rename: Some("sheet'kind".to_string()),
                        is_tuple: true,
                        fields: vec![field("_0", named("SpreadsheetDetails"))],
                        ..EnumVariant::default()
                    },
                    EnumVariant {
                        name: "Named".to_string(),
                        serde_rename: Some("named'kind".to_string()),
                        fields: vec![field("payload", named("WrappedPayload"))],
                        ..EnumVariant::default()
                    },
                    EnumVariant {
                        name: "Empty".to_string(),
                        ..EnumVariant::default()
                    },
                    EnumVariant {
                        name: "Pair".to_string(),
                        fields: vec![field("left", TypeRef::String), field("right", TypeRef::String)],
                        ..EnumVariant::default()
                    },
                ],
                ..EnumDef::default()
            },
            EnumDef {
                name: "DocumentKind".to_string(),
                variants: vec![EnumVariant {
                    name: "Report".to_string(),
                    ..EnumVariant::default()
                }],
                ..EnumDef::default()
            },
            EnumDef {
                name: "AdjacentDetails".to_string(),
                serde_tag: Some("kind".to_string()),
                serde_content: Some("body".to_string()),
                variants: vec![EnumVariant {
                    name: "Wrapped".to_string(),
                    is_tuple: true,
                    fields: vec![field("_0", named("WrappedPayload"))],
                    ..EnumVariant::default()
                }],
                ..EnumDef::default()
            },
        ];
        (type_defs, enums)
    }

    fn resolver() -> FieldResolver {
        let (type_defs, enums) = ir();
        let map = FieldResolver::ir_enum_fields(&type_defs, &enums);
        FieldResolver::new(
            &std::collections::HashMap::new(),
            &std::collections::HashSet::new(),
            &std::collections::HashSet::new(),
            &std::collections::HashSet::new(),
            &std::collections::HashSet::new(),
        )
        .with_ir_enum_map(map, Some("ProcessingResult".to_string()))
        .with_ruby_hash_serialized_enum_names(hash_serialized_enum_names(&enums))
        .with_ruby_typed_variant_payloads(&type_defs, &enums)
        .with_ir_result_fields(
            FieldResolver::ir_result_field_facts(&type_defs, "ruby"),
            Some("ProcessingResult".to_string()),
        )
    }

    /// THE CANARY (positive control). A field path that crosses a hash-serialized enum, spelled
    /// under a name that is nothing like the old literal `metadata.format`, must still refuse —
    /// proving the condition is the enum's own IR-derived lowering, not the field's name.
    #[test]
    fn a_variant_path_through_a_hash_serialized_enum_is_refused_under_any_name() {
        let out = classify(&resolver(), "summary.encoding.spreadsheet.sheet_count");
        assert_eq!(out, RubyEnumAccess::VariantAccessorUnavailable);
    }

    /// Same enum, path stops exactly on it: a `Hash` value does exist, but comparisons against
    /// it as a string/Display value are not meaningful.
    #[test]
    fn a_path_landing_exactly_on_a_hash_serialized_enum_is_flagged_as_hash() {
        let out = classify(&resolver(), "summary.encoding");
        assert_eq!(out, RubyEnumAccess::SerializedAsHash);
    }

    /// THE NEGATIVE CONTROL. `metadata.format` is the exact literal the old guard matched on —
    /// here it resolves through the IR to a plain `String`, not an enum at all, so it must NOT be
    /// refused. This is what tells a real condition apart from a renamed string match.
    #[test]
    fn a_field_that_merely_shares_the_old_literal_name_is_not_refused() {
        let out = classify(&resolver(), "metadata.format");
        assert_eq!(out, RubyEnumAccess::Available);
    }

    /// A `Symbol`-lowered (unit-variant-only) enum has no `Hash` shape to worry about, whatever
    /// the field is named.
    #[test]
    fn a_symbol_lowered_enum_field_is_available() {
        let out = classify(&resolver(), "summary.kind");
        assert_eq!(out, RubyEnumAccess::Available);
    }

    /// An unresolved resolver (no IR wired in at all — the state every resolver had before
    /// `with_ruby_hash_serialized_enum_names` existed) must never refuse: absence of IR data is
    /// "unknown", not "hash-serialized".
    #[test]
    fn with_no_ir_wired_in_nothing_is_ever_refused() {
        let resolver = FieldResolver::new(
            &std::collections::HashMap::new(),
            &std::collections::HashSet::new(),
            &std::collections::HashSet::new(),
            &std::collections::HashSet::new(),
            &std::collections::HashSet::new(),
        );
        assert_eq!(
            classify(&resolver, "summary.encoding.spreadsheet.sheet_count"),
            RubyEnumAccess::Available
        );
        assert_eq!(classify(&resolver, "metadata.format"), RubyEnumAccess::Available);
    }

    /// The partition must reproduce the binding backend's `has_data` split, since it decides both
    /// this skip and the Rust-side `IntoValue` shape.
    #[test]
    fn should_match_the_binding_backends_partition() {
        let (_, enums) = ir();
        let names = hash_serialized_enum_names(&enums);
        assert!(
            names.contains("EncodingDetails"),
            "a data-carrying enum lowers to a Hash"
        );
        assert!(
            !names.contains("DocumentKind"),
            "a unit-variant-only enum lowers to a Symbol"
        );
    }

    fn render(field: &str) -> String {
        let assertion = crate::e2e::fixture::Assertion {
            assertion_type: "equals".to_string(),
            field: Some(field.to_string()),
            value: Some(serde_json::json!("Excel")),
            ..Default::default()
        };
        let mut out = String::new();
        super::super::assertions::render_assertion(
            &mut out,
            &assertion,
            "result",
            &resolver(),
            false,
            &crate::e2e::config::E2eConfig::default(),
            &std::collections::HashSet::new(),
            &std::collections::HashMap::new(),
        );
        out
    }

    #[test]
    fn should_access_a_typed_ruby_variant_through_its_native_payload() {
        let out = render("summary.encoding.spreadsheet.sheet_count");
        assert!(out.contains("enum_value.spreadsheet?"), "{out}");
        assert!(out.contains("enum_value.value.sheet_count"), "{out}");
        assert!(!out.contains(".fetch("), "{out}");
    }

    /// A serde-renamed variant still uses its Rust-derived native predicate.
    #[test]
    fn render_assertion_reaches_a_native_variant_field_under_any_name() {
        let out = render("summary.encoding.spreadsheet.sheet_count");
        assert!(
            out.contains("result.summary.encoding.then { |enum_value|"),
            "got: {out}"
        );
        assert!(out.contains("enum_value.spreadsheet?"), "got: {out}");
        assert!(out.contains("enum_value.value.sheet_count"), "got: {out}");
        assert!(!out.contains("fetch('_0'.to_sym)"), "got: {out}");
        assert!(!out.contains("# skipped:"), "got: {out}");
    }

    #[test]
    fn render_assertion_omits_the_wrapper_hop_for_a_flattened_tuple_variant() {
        let out = render("summary.encoding.spreadsheet.sheet_count");
        assert!(out.contains("enum_value.value.sheet_count"), "got: {out}");
        assert!(!out.contains("fetch('_0'.to_sym)"), "got: {out}");
    }

    /// The other side of the same flag, and the reason it cannot simply be deleted: a STRUCT
    /// variant (one NAMED field) is not flattened by `backends::magnus`, so its payload really does
    /// nest one Hash level deeper and the hop must still be emitted.
    #[test]
    fn render_assertion_keeps_the_wrapper_hop_for_a_named_single_field_variant() {
        let out = render("summary.encoding.named.value");
        assert!(
            out.contains("enum_hash.fetch('payload'.to_sym).fetch('value'.to_sym)"),
            "got: {out}"
        );
    }

    /// Native payload objects do not depend on serde's configured content key.
    #[test]
    fn render_assertion_uses_the_native_payload_for_adjacently_tagged_enums() {
        let out = render("summary.adjacent.wrapped.value");
        assert!(out.contains("enum_value.value.value"), "got: {out}");
        assert!(!out.contains("fetch('_0'.to_sym)"), "got: {out}");
    }

    /// A supported single-field payload is executable and therefore absent from the skip ledger.
    #[test]
    fn render_assertion_supported_variant_field_has_no_skip_classification() {
        use crate::e2e::codegen::field_skip::FieldSkip;
        let out = render("summary.encoding.spreadsheet.sheet_count");
        assert_eq!(FieldSkip::extract_classified(&out), None, "got: {out}");
    }

    #[test]
    fn runtime_accessor_rejects_a_different_wire_variant() {
        let out = render("summary.encoding.spreadsheet.sheet_count");
        assert!(
            out.contains("raise \"unexpected tagged enum variant\" unless"),
            "got: {out}"
        );
        assert!(out.contains("enum_value.spreadsheet?"), "got: {out}");
    }

    #[test]
    fn should_keep_hash_access_for_opaque_and_trait_payloads() {
        for (opaque, is_trait) in [(true, false), (false, true)] {
            let (mut types, enums) = ir();
            let payload = types.iter_mut().find(|typ| typ.name == "SpreadsheetDetails").unwrap();
            payload.is_opaque = opaque;
            payload.is_trait = is_trait;
            let resolver = resolver().with_ruby_typed_variant_payloads(&types, &enums);
            let out =
                super::variant_field_accessor(&resolver, "summary.encoding.spreadsheet.sheet_count", "result").unwrap();
            assert!(out.contains("enum_hash.fetch('sheet_count'.to_sym)"), "{out}");
            assert!(!out.contains("enum_value.value"), "{out}");
        }
    }

    #[test]
    fn nonexistent_payload_leaf_remains_a_generator_gap() {
        let out = render("summary.encoding.spreadsheet.not_declared");
        assert!(out.contains("# skipped: enum variant accessor"), "got: {out}");
        assert!(!out.contains("fetch(\"not_declared\".to_sym)"), "got: {out}");
    }

    #[test]
    fn nested_and_indexed_payload_suffixes_remain_generator_gaps() {
        for field in [
            "summary.encoding.spreadsheet.sheet_count.value",
            "summary.encoding.spreadsheet.sheet_count[0]",
        ] {
            let out = render(field);
            assert!(
                out.contains("# skipped: enum variant accessor"),
                "field={field}, got: {out}"
            );
        }
    }

    #[test]
    fn unknown_variant_remains_a_generator_gap() {
        for field in [
            "summary.encoding.unknown.sheet_count",
            "summary.encoding.empty.sheet_count",
            "summary.encoding.pair.left",
        ] {
            let out = render(field);
            assert!(
                out.contains("# skipped: enum variant accessor"),
                "field={field}, got: {out}"
            );
        }
    }
}

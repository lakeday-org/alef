use super::*;
use crate::core::ir::{FieldDef, PrimitiveType};

fn field(name: &str, ty: TypeRef) -> FieldDef {
    FieldDef {
        name: name.into(),
        optional: matches!(ty, TypeRef::Optional(_)),
        ty,
        ..Default::default()
    }
}
fn optional(ty: TypeRef) -> TypeRef {
    TypeRef::Optional(Box::new(ty))
}
fn types(optional_details: bool) -> Vec<TypeDef> {
    let details = TypeRef::Named("Details".into());
    vec![
        TypeDef {
            name: "Chunk".into(),
            has_serde: true,
            fields: vec![field("usage", optional(TypeRef::Named("Usage".into())))],
            ..Default::default()
        },
        TypeDef {
            name: "Usage".into(),
            has_serde: true,
            fields: vec![
                field("details", if optional_details { optional(details) } else { details }),
                field("total_tokens", TypeRef::Primitive(PrimitiveType::U64)),
            ],
            ..Default::default()
        },
        TypeDef {
            name: "Details".into(),
            has_serde: true,
            fields: vec![field("cached_tokens", TypeRef::Primitive(PrimitiveType::U64))],
            ..Default::default()
        },
    ]
}

#[test]
fn typed_usage_chains_only_declared_optionals() {
    for language in [Language::Swift, Language::Dart] {
        for optional_details in [false, true] {
            let types = types(optional_details);
            let config = ResolvedCrateConfig::default();
            let context = UsageAccessContext {
                language,
                item_type: "Chunk",
                types: &types,
                config: &config,
                swift_map: None,
            };
            let expression = context
                .accessor("usage.details.cached_tokens", "chunks")
                .expect("typed path");
            let last = if language == Language::Swift {
                "last"
            } else {
                "lastOrNull"
            };
            assert_eq!(
                expression,
                format!(
                    "chunks.{last}?.usage?.details{}cachedTokens",
                    if optional_details { "?." } else { "." }
                )
            );
            assert_eq!(
                context.accessor("usage.total_tokens", "chunks"),
                Some(format!("chunks.{last}?.usage?.totalTokens"))
            );
        }
    }
}

#[test]
fn excluded_paths_are_not_fabricated() {
    let mut types = types(false);
    types[1].fields[0].binding_excluded = true;
    let config = ResolvedCrateConfig::default();
    let context = UsageAccessContext {
        language: Language::Swift,
        item_type: "Chunk",
        types: &types,
        config: &config,
        swift_map: None,
    };
    assert!(context.accessor("usage.details.cached_tokens", "chunks").is_none());
    assert!(context.accessor("plain.details", "chunks").is_none());
}

#[test]
#[ignore = "requires Swift and Dart"]
fn swift_and_dart_compile_required_and_optional_nested_usage() {
    use crate::e2e::codegen::streaming_assertions::probe_support::{Probe, assert_status};
    let config = ResolvedCrateConfig::default();
    let mut executed = 0;
    for (language, runner, template) in [
        (Language::Swift, "swift", "swift/usage_runtime_probe.jinja"),
        (Language::Dart, "dart", "dart/usage_runtime_probe.jinja"),
    ] {
        let probe = Probe::new(runner);
        for optional_details in [false, true] {
            let types = types(optional_details);
            let context = UsageAccessContext {
                language,
                item_type: "Chunk",
                types: &types,
                config: &config,
                swift_map: None,
            };
            let source = crate::e2e::template_env::render(
                template,
                crate::alef_context! {
                    optional => if optional_details {"?"} else {""},
                    expression => context.accessor("usage.details.cached_tokens","chunks").expect("nested"),
                    standard => context.accessor("usage.total_tokens","chunks").expect("standard"),
                    absent => context.accessor("usage.details.cached_tokens","absent").expect("absent"),
                },
            );
            assert_status(&probe.run(&source), true);
            executed += 1;
        }
    }
    assert_eq!(executed, 4);
    let old = crate::e2e::template_env::render(
        "swift/usage_runtime_probe.jinja",
        crate::alef_context! {
            optional => "", expression => "chunks.last?.usage?.details?.cachedTokens",
            standard => "chunks.last?.usage?.totalTokens", absent => "absent.last?.usage?.details.cachedTokens",
        },
    );
    let output = Probe::new("swift").run(&old);
    assert_status(&output, false);
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot use optional chaining"));
}

#[test]
fn full_generators_use_typed_stream_usage_paths() {
    use crate::e2e::codegen::{E2eCodegen, dart::DartE2eCodegen, swift::SwiftE2eCodegen};
    for optional_details in [false, true] {
        let (config, e2e, groups) = generation_inputs();
        let types = types(optional_details);
        for (generator, prefix) in [
            (&SwiftE2eCodegen as &dyn E2eCodegen, "chunks.last"),
            (&DartE2eCodegen as &dyn E2eCodegen, "result.lastOrNull"),
        ] {
            let files = generator
                .generate(&groups, &e2e, &config, &types, &[], &[], &[])
                .expect("generation");
            let expected = format!(
                "{prefix}?.usage?.details{}cachedTokens",
                if optional_details { "?." } else { "." }
            );
            assert!(
                files.iter().any(|file| file.content.contains(&expected)),
                "missing {expected}: {files:?}"
            );
        }
    }
}

fn generation_inputs() -> (
    ResolvedCrateConfig,
    crate::e2e::config::E2eConfig,
    Vec<crate::e2e::fixture::FixtureGroup>,
) {
    use crate::core::config::e2e::{CallOverride, StreamingConfig};
    let mut config = ResolvedCrateConfig {
        name: "sample".into(),
        ..Default::default()
    };
    config.adapters.push(
        serde_json::from_value(serde_json::json!({
            "name":"stream_values","pattern":"streaming","core_path":"sample::Service::stream_values",
            "owner_type":"Service","item_type":"Chunk"
        }))
        .expect("adapter"),
    );
    let mut e2e = crate::e2e::config::E2eConfig::default();
    e2e.call.function = "stream_values".into();
    e2e.call.streaming = Some(StreamingConfig::Enabled(true));
    for language in ["swift", "dart"] {
        e2e.call.overrides.insert(
            language.into(),
            CallOverride {
                function: Some(
                    if language == "swift" {
                        "streamValues"
                    } else {
                        "stream_values"
                    }
                    .into(),
                ),
                client_factory: Some("Service".into()),
                result_type: Some("Chunk".into()),
                ..Default::default()
            },
        );
    }
    let fixture = serde_json::from_value(serde_json::json!({
        "id":"typed_usage","input":null,"assertions":[{"type":"equals","field":"usage.details.cached_tokens","value":18}]
    })).expect("fixture");
    let groups = vec![crate::e2e::fixture::FixtureGroup {
        category: "usage".into(),
        fixtures: vec![fixture],
    }];
    (config, e2e, groups)
}

#[test]
fn usage_paths_keep_wire_names_and_public_renames_separate() {
    for language in [Language::Swift, Language::Dart] {
        let mut types = types(false);
        types[2].fields[0].serde_rename = Some("cached-count".into());
        let mut config = ResolvedCrateConfig::default();
        match language {
            Language::Swift => {
                config
                    .swift
                    .get_or_insert_with(Default::default)
                    .rename_fields
                    .insert("Details.cached_tokens".into(), "stored_tokens".into());
            }
            _ => {
                config
                    .dart
                    .get_or_insert_with(Default::default)
                    .rename_fields
                    .insert("Details.cached_tokens".into(), "stored_tokens".into());
            }
        }
        let context = UsageAccessContext {
            language,
            item_type: "Chunk",
            types: &types,
            config: &config,
            swift_map: None,
        };
        assert!(
            context
                .accessor("usage.details.cached-count", "chunks")
                .expect("wire alias")
                .ends_with("details.storedTokens")
        );
    }
}

#[test]
fn typed_unavailable_paths_emit_ledger_markers_without_hiding_custom_accessors() {
    let mut types = types(false);
    types[1].fields[0].binding_excluded = true;
    let config = ResolvedCrateConfig::default();
    let context = UsageAccessContext {
        language: Language::Swift,
        item_type: "Chunk",
        types: &types,
        config: &config,
        swift_map: None,
    };
    let assertion = crate::e2e::fixture::Assertion {
        assertion_type: "equals".into(),
        field: Some("usage.details.cached_tokens".into()),
        value: Some(serde_json::json!(18)),
        ..Default::default()
    };
    let paths = UsagePathOverrides::prepare(
        &Default::default(),
        std::slice::from_ref(&assertion),
        Some(&context),
        "chunks",
    );
    let mut source = String::new();
    assert!(paths.render_unavailable(&mut source, &assertion, "        "));
    assert!(source.contains("// skipped: "), "{source}");
    let initial = std::collections::HashMap::from([("usage.details.cached_tokens".into(), "customCounter".into())]);
    let paths = UsagePathOverrides::prepare(&initial, &[assertion], Some(&context), "chunks");
    assert_eq!(paths.expressions["usage.details.cached_tokens"], "customCounter");
}

#[test]
fn swift_dto_optionality_is_independent_of_opaque_json_getter_transport() {
    let types = types(true);
    let config = ResolvedCrateConfig::default();
    let map = SwiftFirstClassMap {
        first_class_types: ["Chunk", "Usage", "Details"].into_iter().map(str::to_string).collect(),
        getter_optionality: std::collections::HashMap::from([
            (
                "Chunk".into(),
                std::collections::HashMap::from([("usage".into(), false)]),
            ),
            (
                "Usage".into(),
                std::collections::HashMap::from([("details".into(), false)]),
            ),
        ]),
        ..Default::default()
    };
    let context = UsageAccessContext {
        language: Language::Swift,
        item_type: "Chunk",
        types: &types,
        config: &config,
        swift_map: Some(&map),
    };
    assert_eq!(
        context.accessor("usage.details.cached_tokens", "chunks").as_deref(),
        Some("chunks.last?.usage?.details?.cachedTokens")
    );
}

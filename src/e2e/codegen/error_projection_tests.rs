use super::declared_error_variant::{DeclaredErrorAssertion, classify, declared_variant};
use crate::codegen::binding_projection::{project, project_errors};
use crate::core::ir::{ApiSurface, ErrorDef, ErrorVariant, MethodDef, PrimitiveType, TypeRef};
use crate::e2e::config::{CallConfig, E2eConfig};
use crate::e2e::fixture::{Assertion, Fixture, FixtureGroup};

fn errors() -> Vec<ErrorDef> {
    [("HiddenError", true), ("ApiError", false)]
        .into_iter()
        .map(|(name, binding_excluded)| ErrorDef {
            name: name.into(),
            binding_excluded,
            exception_base: None,
            rust_path: format!("sample::{name}"),
            original_rust_path: String::new(),
            doc: String::new(),
            methods: vec![MethodDef {
                name: "status_code".into(),
                return_type: TypeRef::Primitive(PrimitiveType::U16),
                ..Default::default()
            }],
            binding_exclusion_reason: None,
            version: Default::default(),
            variants: vec![ErrorVariant {
                name: "Connection".into(),
                error_code: Some(1001),
                message_template: Some("connection failed".into()),
                is_unit: true,
                ..Default::default()
            }],
        })
        .collect()
}

fn fixture() -> Fixture {
    Fixture {
        id: "error_projection".into(),
        assertions: vec![Assertion {
            assertion_type: "error".into(),
            value: Some(serde_json::json!("Connection")),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn e2e_and_bindings_share_the_same_projection_and_borrow_visible_registries() {
    let errors = errors();
    assert_eq!(errors.len(), 2);
    let projected = project_errors(&errors);
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].name, "ApiError");
    let bindings = project(&ApiSurface {
        errors: errors.clone(),
        ..Default::default()
    });
    assert_eq!(
        serde_json::to_value(&*projected).unwrap(),
        serde_json::to_value(bindings.errors).unwrap()
    );
    assert!(matches!(project_errors(&projected), std::borrow::Cow::Borrowed(_)));
}

#[test]
fn direct_classifiers_ignore_hidden_variants_and_resolve_visible_collisions() {
    let errors = errors();
    let fixture = fixture();
    assert_eq!(declared_variant(&fixture, &errors).unwrap().0.name, "ApiError");
    for language in ["ruby", "go", "java", "csharp", "dart", "php", "swift"] {
        assert!(declared_variant(&fixture, &errors[..1]).is_none(), "{language}");
        assert_eq!(
            classify(language, &fixture, &errors[..1]),
            DeclaredErrorAssertion::Assert("Connection"),
            "hidden variants remain literal messages for {language}",
        );
        assert!(super::snippet_error_branch::for_fixture(language, &fixture, &errors[..1]).is_none());
    }
}

#[test]
fn direct_snippets_ignore_hidden_errors_in_variant_naming_collisions() {
    let errors = errors();
    let projected = project_errors(&errors);
    let fixture = fixture();
    for language in ["ruby", "go", "dart", "java", "python", "zig"] {
        let raw = super::snippet_error_branch::for_fixture(language, &fixture, &errors);
        assert!(raw.is_some(), "a coded visible variant must render for {language}");
        assert_eq!(
            raw,
            super::snippet_error_branch::for_fixture(language, &fixture, &projected),
            "{language}",
        );
    }
}

#[test]
fn direct_public_generators_match_the_binding_projected_error_registry() {
    let errors = errors();
    let projected = project_errors(&errors);
    let groups = [FixtureGroup {
        category: "errors".into(),
        fixtures: vec![fixture()],
    }];
    let e2e = E2eConfig {
        call: CallConfig {
            function: "read_value".into(),
            module: "Sample".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let config = crate::core::config::ResolvedCrateConfig {
        name: "sample".into(),
        ..Default::default()
    };
    let languages = ["ruby", "go", "java", "csharp", "dart", "php"];
    let generators = super::generators_for(&languages.map(str::to_string));
    assert_eq!(generators.len(), languages.len());
    for generator in generators {
        let raw = generator
            .generate(&groups, &e2e, &config, &[], &[], &[], &errors)
            .unwrap();
        let visible = generator
            .generate(&groups, &e2e, &config, &[], &[], &[], &projected)
            .unwrap();
        assert!(!raw.is_empty(), "{} must emit files", generator.language_name());
        let artifacts = |files: Vec<crate::core::backend::GeneratedFile>| {
            files
                .into_iter()
                .map(|file| (file.path, file.content))
                .collect::<Vec<_>>()
        };
        assert_eq!(artifacts(raw), artifacts(visible), "{}", generator.language_name());
    }
}

fn python_case() -> (Fixture, E2eConfig, crate::core::config::ResolvedCrateConfig) {
    let mut fixture = fixture();
    fixture.assertions.push(Assertion {
        assertion_type: "equals".into(),
        field: Some("error.status_code".into()),
        value: Some(serde_json::json!(400)),
        ..Default::default()
    });
    let e2e = E2eConfig {
        call: CallConfig {
            function: "read_value".into(),
            module: "sample".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let config = crate::core::config::ResolvedCrateConfig {
        name: "sample".into(),
        ..Default::default()
    };
    (fixture, e2e, config)
}

#[test]
fn direct_python_error_fields_and_snippets_use_only_visible_error_types() {
    use super::E2eCodegen;
    let errors = errors();
    let projected = project_errors(&errors);
    let (fixture, e2e, config) = python_case();
    let groups = [FixtureGroup {
        category: "errors".into(),
        fixtures: vec![fixture.clone()],
    }];
    let generator = super::python::PythonE2eCodegen;
    let render = |registry: &[ErrorDef]| {
        generator
            .generate(&groups, &e2e, &config, &[], &[], &[], registry)
            .unwrap()
            .into_iter()
            .map(|file| (file.path, file.content))
            .collect::<Vec<_>>()
    };
    let raw = render(&errors);
    assert!(
        raw.iter()
            .any(|(_, source)| source.contains("sample.api_error_info(exc_info.value).status_code == 400")),
        "{raw:?}"
    );
    assert_eq!(raw, render(&projected));
    let snippet = |registry: &[ErrorDef]| {
        generator
            .render_snippet_body_with_functions(&fixture, &e2e, &config, &[], &[], &[], registry)
            .unwrap()
    };
    let source = snippet(&errors);
    assert!(
        source.contains("from sample import ApiConnectionError, ApiError"),
        "{source}"
    );
    assert_eq!(source, snippet(&projected));
    assert!(!source.contains("HiddenError"), "{source}");
}

#[test]
fn hidden_introspection_types_never_resolve_an_error_field() {
    let errors = errors();
    let lookup = |registry| {
        super::error_field_reachability::introspection_method(registry, "status_code")
            .map(|(error, _)| error.name.clone())
    };
    assert_eq!(lookup(&errors), Some("ApiError".into()));
    assert_eq!(lookup(&errors[..1]), None);
}

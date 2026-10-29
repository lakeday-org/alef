use alef::{
    backends::pyo3::Pyo3Backend,
    core::{backend::Backend, config::NewAlefConfig, ir::*},
};

fn config() -> alef::core::config::ResolvedCrateConfig {
    let config: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["python"]
[[crates]]
name = "sample-core"
sources = ["src/lib.rs"]
[crates.python.stubs]
output = "packages/python"
"#,
    )
    .unwrap();
    config.resolve().unwrap().remove(0)
}

#[test]
fn should_export_keyword_method_names_advertised_by_python_stubs() {
    let config = config();
    let api = ApiSurface {
        types: vec![TypeDef {
            name: "Check".into(),
            rust_path: "sample_core::Check".into(),
            is_clone: true,
            methods: [("pass", true), ("global", true), ("lambda", false)]
                .into_iter()
                .map(|(name, is_static)| MethodDef {
                    name: name.into(),
                    return_type: TypeRef::Unit,
                    is_static,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let files = Pyo3Backend.generate_bindings(&api, &config).unwrap();
    let code = &files.iter().find(|file| file.path.ends_with("lib.rs")).unwrap().content;
    let stubs = Pyo3Backend.generate_type_stubs(&api, &config).unwrap();
    for method in ["pass", "global", "lambda"] {
        assert!(
            stubs[0].content.contains(&format!("def {method}_(")),
            "{}",
            stubs[0].content
        );
        assert!(code.contains(&format!("#[pyo3(name = \"{method}_\")]")), "{code}");
    }
    syn::parse_file(code).expect("generated bindings must remain valid Rust");
}

#[test]
fn should_omit_unreachable_foreign_enum_variants_from_python_stubs() {
    let config = config();
    let api = ApiSurface {
        enums: vec![EnumDef {
            name: "TierStrategy".into(),
            rust_path: "dependency::TierStrategy".into(),
            variants: vec![
                EnumVariant {
                    name: "Auto".into(),
                    ..Default::default()
                },
                EnumVariant {
                    name: "Tier1".into(),
                    cfg: Some("any(test, feature = \"testkit\")".into()),
                    ..Default::default()
                },
                EnumVariant {
                    name: "Tier2".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let files = Pyo3Backend.generate_bindings(&api, &config).unwrap();
    let code = &files.iter().find(|file| file.path.ends_with("lib.rs")).unwrap().content;
    assert!(
        !code.contains("name = \"TIER1\""),
        "native declaration must omit unreachable variants: {code}"
    );
    let stubs = Pyo3Backend.generate_type_stubs(&api, &config).unwrap();
    let stub = &stubs[0].content;
    assert!(stub.contains("AUTO: TierStrategy"), "{stub}");
    assert!(stub.contains("TIER2: TierStrategy"), "{stub}");
    assert!(
        !stub.contains("TIER1: TierStrategy"),
        "stub must match native declarations: {stub}"
    );
}

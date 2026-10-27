use super::tests::make_param;
use super::visitor::gen_convert_with_visitor_wrapper;
use crate::backends::go::gen_visitor::gen_visitor_file;
use crate::core::config::{BridgeBinding, TraitBridgeConfig};
use crate::core::ir::*;
use crate::test_support::toolchain;
use std::collections::HashSet;

fn policy_function(name: &str, with_flag: bool) -> FunctionDef {
    let mut params = vec![make_param("document", TypeRef::String)];
    if with_flag {
        params.push(make_param("enabled", TypeRef::Primitive(PrimitiveType::Bool)));
    }
    let mut options = make_param(
        "settings",
        TypeRef::Optional(Box::new(TypeRef::Named("RenderOptions".into()))),
    );
    options.optional = true;
    params.push(options);
    FunctionDef {
        name: name.into(),
        params,
        return_type: TypeRef::Named("RenderOutput".into()),
        error_type: Some("Error".into()),
        ..FunctionDef::default()
    }
}

fn policy_fixture() -> (ApiSurface, TypeDef, TraitBridgeConfig) {
    let bridge = TraitBridgeConfig {
        trait_name: "Renderer".into(),
        bind_via: BridgeBinding::OptionsField,
        options_type: Some("RenderOptions".into()),
        options_field: Some("renderer".into()),
        context_type: Some("Context".into()),
        result_type: Some("Decision".into()),
        ..TraitBridgeConfig::default()
    };
    let visitor = TypeDef {
        name: "Renderer".into(),
        is_trait: true,
        methods: vec![MethodDef {
            name: "visit_text".into(),
            params: vec![
                make_param("ctx", TypeRef::Named("Context".into())),
                make_param("text", TypeRef::String),
            ],
            return_type: TypeRef::Named("Decision".into()),
            ..MethodDef::default()
        }],
        ..TypeDef::default()
    };
    let base = policy_function("render", false);
    let api = ApiSurface {
        types: vec![TypeDef {
            name: "Context".into(),
            ..TypeDef::default()
        }],
        enums: vec![EnumDef {
            name: "Decision".into(),
            variants: vec![EnumVariant {
                name: "Continue".into(),
                is_default: true,
                ..EnumVariant::default()
            }],
            ..EnumDef::default()
        }],
        functions: vec![base.clone(), policy_function("render_with_policy", true)],
        ..ApiSurface::default()
    };
    (api, visitor, bridge)
}

fn generated_policy_functions() -> String {
    let (api, visitor, bridge) = policy_fixture();
    let visitor_code = gen_visitor_file(
        &api,
        "sample",
        "krz",
        "unused.h",
        ".",
        ".",
        "Renderer",
        "renderer",
        &visitor,
        &bridge,
        &api.functions[0],
    );
    let mut generated = String::new();
    for function in &api.functions {
        generated.push_str(&gen_convert_with_visitor_wrapper(
            function,
            "krz",
            &HashSet::new(),
            &HashSet::new(),
            &bridge,
            &HashSet::new(),
        ));
        let name = crate::codegen::naming::go_param_name(&function.name);
        let marker = format!("func {name}WithVisitorHelper(");
        if let Some(start) = visitor_code.find(&marker) {
            let end = visitor_code[start..].find("\n}\n").expect("helper end") + start + 3;
            generated.push_str(&visitor_code[start..end]);
        }
    }
    generated
}

#[test]
#[allow(
    clippy::print_stderr,
    reason = "Expose the six native Go cases in captured test evidence"
)]
fn generated_options_policy_runs_both_flags_through_plain_and_visitor_paths() {
    let Some(go) = toolchain::GO.open() else {
        return;
    };
    let generated = generated_policy_functions();
    let directory = tempfile::tempdir().expect("Go policy fixture");
    std::fs::write(
        directory.path().join("go.mod"),
        "module example.com/policy\n\ngo 1.24\n",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("policy.go"),
        format!("{}\n{generated}", include_str!("visitor_policy_runtime.go.fixture")),
    )
    .unwrap();
    std::fs::write(
        directory.path().join("policy_test.go"),
        include_str!("visitor_policy_runtime_test.go.fixture"),
    )
    .unwrap();
    let output = std::process::Command::new(go)
        .args(["test", "-v", "-count=1", "./..."])
        .current_dir(directory.path())
        .output()
        .expect("execute generated Go");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Go policy fixture failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{stdout}");
    assert_eq!(
        stdout.matches("--- PASS: TestPolicy/").count(),
        6,
        "six runtime cases must execute: {stdout}"
    );
}

#[test]
fn excluded_first_options_function_does_not_emit_a_visitor_helper() {
    use crate::core::backend::Backend;
    let config: crate::core::config::NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["ffi", "go"]
[[crates]]
name = "sample"
sources = ["src/lib.rs"]
[crates.ffi]
prefix = "krz"
visitor_callbacks = true
exclude_functions = ["render"]
[crates.go]
module = "example.com/sample"
"#,
    )
    .unwrap();
    let mut config = config.resolve().unwrap().remove(0);
    let (mut api, visitor, bridge) = policy_fixture();
    api.types.push(visitor);
    config.replace_trait_bridges(vec![bridge]);
    let files = crate::backends::go::GoBackend.generate_bindings(&api, &config).unwrap();
    let visitor = &files
        .iter()
        .find(|file| file.path.ends_with("visitor.go"))
        .unwrap()
        .content;
    assert_eq!(visitor.matches("func renderWithPolicyWithVisitorHelper(").count(), 1);
    assert!(!visitor.contains("func renderWithVisitorHelper("));
}

use super::legacy_cell;
use crate::backends::napi::type_map::NapiMapper;
use crate::codegen::conversions::ConversionConfig;
use std::process::Command;

const CORE_AND_MAIN: &str = r#"
mod sample_core {
    #[derive(Default)]
    pub struct GridCell {
        pub content: String,
        pub row: u32,
        pub label: Option<String>,
        pub annotations: Vec<String>,
        pub timeout: std::time::Duration,
        pub delay: std::time::Duration,
    }
}
fn main() {
    let binding = JsGridCell {
        content: "kept".into(), row: 7, label: Some("label".into()), annotations: None, timeout: 1250, delay: None,
    };
    let core: sample_core::GridCell = binding.into();
    assert_eq!(core.content, "kept");
    assert_eq!(core.row, 7);
    assert_eq!(core.label.as_deref(), Some("label"));
    assert!(core.annotations.is_empty());
    assert_eq!(core.timeout.as_millis(), 1250);
    assert!(core.delay.is_zero());
    let binding: JsGridCell = core.into();
    assert_eq!(binding.content, "kept");
    assert_eq!(binding.row, 7);
    assert_eq!(binding.label.as_deref(), Some("label"));
    assert_eq!(binding.annotations, Some(Vec::new()));
    assert_eq!(binding.timeout, 1250);
    assert_eq!(binding.delay, Some(0));
}
"#;

fn legacy_duration_cell(has_default: bool) -> crate::core::ir::TypeDef {
    let mut typ = legacy_cell(has_default);
    for name in ["timeout", "delay"] {
        typ.fields.push(crate::core::ir::FieldDef {
            name: name.into(),
            ty: crate::core::ir::TypeRef::Duration,
            ..Default::default()
        });
    }
    typ.version.legacy_constructor_fields.push("timeout".into());
    typ
}

fn generated_round_trip(has_default: bool) -> String {
    let typ = legacy_duration_cell(has_default);
    let config = ConversionConfig {
        type_name_prefix: "Js",
        optionalize_defaults: true,
        optionalize_bare_field_defaults: true,
        option_duration_on_defaults: true,
        cast_large_ints_to_i64: true,
        cast_f32_to_f64: true,
        ..Default::default()
    };
    let rendered = super::super::gen_struct(
        &typ,
        &NapiMapper::new("Js".into()),
        "Js",
        true,
        &Default::default(),
        &[],
        &[],
        "sample_core",
        &Default::default(),
        None,
        &[],
    );
    let mut parsed = syn::parse_file(&rendered).expect("generated Rust syntax");
    for item in &mut parsed.items {
        if let syn::Item::Struct(item) = item {
            item.attrs.clear();
            for field in &mut item.fields {
                field.attrs.clear();
            }
        }
    }
    let to_core = crate::codegen::conversions::gen_from_binding_to_core_cfg(&typ, "sample_core", &config);
    let from_core =
        crate::codegen::conversions::gen_from_core_to_binding_cfg(&typ, "sample_core", &Default::default(), &config);
    [
        quote::quote!(#parsed).to_string(),
        to_core,
        from_core,
        CORE_AND_MAIN.into(),
    ]
    .join("\n")
}

fn compile_probe(dir: &std::path::Path, source: &std::path::Path, binary: &std::path::Path) -> std::process::Output {
    Command::new("rustc")
        .current_dir(dir)
        .args(["--edition=2024", "--crate-name", "probe"])
        .arg(source)
        .arg("-o")
        .arg(binary)
        .output()
        .expect("compile generated probe")
}

#[test]
fn legacy_constructor_round_trip_compiles_and_preserves_fields() {
    let dir = tempfile::tempdir().expect("owned probe directory");
    let source = dir.path().join("probe.rs");
    let binary = dir.path().join(format!("probe{}", std::env::consts::EXE_SUFFIX));
    for has_default in [true, false] {
        let rendered = generated_round_trip(has_default);
        assert!(rendered.contains("pub content : String"), "{rendered}");
        let broken = rendered.replacen("pub content : String", "pub content : Option<String>", 1);
        assert_ne!(rendered, broken);
        std::fs::write(&source, broken).expect("write negative control");
        let rejected = compile_probe(dir.path(), &source, &binary);
        assert!(!rejected.status.success());
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("E0308"));
        std::fs::write(&source, rendered).expect("write generated probe");
        let compiled = compile_probe(dir.path(), &source, &binary);
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&binary)
            .current_dir(dir.path())
            .output()
            .expect("run generated probe");
        assert!(
            executed.status.success(),
            "{}",
            String::from_utf8_lossy(&executed.stderr)
        );
    }
}

#[test]
fn duration_only_policy_preserves_defaulted_legacy_timeouts() {
    let typ = legacy_duration_cell(true);
    let timeout = typ.fields.iter().find(|f| f.name == "timeout").expect("legacy timeout");
    let delay = typ.fields.iter().find(|f| f.name == "delay").expect("extension delay");
    for (regular_defaults, legacy_optional) in [(false, true), (true, false)] {
        let config = ConversionConfig {
            option_duration_on_defaults: true,
            optionalize_defaults: regular_defaults,
            ..Default::default()
        };
        assert_eq!(config.duration_field_is_optionalized(&typ, timeout), legacy_optional);
        assert!(config.duration_field_is_optionalized(&typ, delay));
    }
}

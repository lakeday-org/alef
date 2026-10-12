use alef::core::backend::Backend;
use alef::core::config::{ResolvedCrateConfig, new_config::NewAlefConfig};
use alef::core::ir::ApiSurface;

fn api(annotated: bool, extension: bool, default: bool) -> ApiSurface {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lib.rs");
    let fields = (0..12)
        .map(|index| format!("pub flag_{index}: bool,"))
        .collect::<Vec<_>>();
    let mut names = (0..8).map(|index| format!("\"flag_{index}\"")).collect::<Vec<_>>();
    names.push("\"max_depth\"".into());
    names.extend((8..12).map(|index| format!("\"flag_{index}\"")));
    let history = if annotated {
        format!("#[alef(legacy_constructor_fields = [{}])]", names.join(", "))
    } else {
        String::new()
    };
    let added = if extension { "pub keep_hidden: bool," } else { "" };
    let all_fields = format!(
        "{} pub max_depth: Option<usize>, {} {added}",
        fields[..8].join(" "),
        fields[8..].join(" ")
    );
    let values = (0..12).map(|index| format!("flag_{index}: false,")).collect::<String>();
    std::fs::write(
        &path,
        format!(
            r#"
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum Payload {{ Value {{ enabled: bool }} }}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
{history}
pub struct Options {{ {all_fields} }}
impl Default for Options {{ fn default() -> Self {{ Self {{ {values} max_depth: None, {} }} }} }}
"#,
            if extension {
                format!("keep_hidden: {default},")
            } else {
                String::new()
            }
        ),
    )
    .unwrap();
    alef::extract::extractor::extract(&[&path], "sample", "1.0.0", None).unwrap()
}

fn config() -> ResolvedCrateConfig {
    let raw: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["php", "wasm", "zig"]
[[crates]]
name = "sample"
sources = ["src/lib.rs"]
"#,
    )
    .unwrap();
    raw.resolve().unwrap().remove(0)
}

fn render(backend: &dyn Backend, api: &ApiSurface) -> String {
    let files = backend.generate_bindings(api, &config()).unwrap();
    assert!(!files.is_empty());
    files
        .into_iter()
        .map(|file| file.content)
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_published_slots(backend: &dyn Backend) {
    let source = render(backend, &api(true, true, false));
    let signature = regex::Regex::new(r"pub fn new\(([^)]*)\)")
        .unwrap()
        .captures(&source)
        .unwrap_or_else(|| panic!("missing constructor: {source}"))[1]
        .to_string();
    let names = signature
        .split(',')
        .filter(|parameter| !parameter.trim().is_empty())
        .map(|parameter| parameter.trim().split(':').next().unwrap())
        .collect::<Vec<_>>();
    let mut expected = (0..12).map(|index| format!("flag{index}")).collect::<Vec<_>>();
    expected.extend(["maxDepth".into(), "keepHidden".into()]);
    assert_eq!(names, expected, "{signature}");
    assert!(signature.contains("keepHidden: Option<bool>"), "{signature}");
    assert!(source.contains("keep_hidden: keepHidden.unwrap_or(false)"), "{source}");
}

#[test]
fn should_keep_php_published_optional_slot_before_appended_extension() {
    assert_published_slots(&alef::backends::php::PhpBackend);
}

#[test]
fn should_keep_wasm_published_optional_slot_before_appended_extension() {
    assert_published_slots(&alef::backends::wasm::WasmBackend);
}

#[test]
fn should_default_only_annotated_zig_extension_to_extracted_rust_value() {
    for default in [false, true] {
        let source = render(&alef::backends::zig::ZigBackend, &api(true, true, default));
        assert!(source.contains(&format!("keep_hidden: bool = {default},")), "{source}");
        assert!(source.contains("flag_0: bool,"), "{source}");
        let plain = render(&alef::backends::zig::ZigBackend, &api(false, true, default));
        assert!(plain.contains("keep_hidden: bool,"), "{plain}");
    }
}

#[test]
fn should_preserve_unextended_php_and_wasm_generated_bytes_when_history_is_added() {
    for backend in [
        &alef::backends::php::PhpBackend as &dyn Backend,
        &alef::backends::wasm::WasmBackend,
        &alef::backends::zig::ZigBackend,
    ] {
        assert_eq!(
            render(backend, &api(true, false, false)),
            render(backend, &api(false, false, false))
        );
    }
}

#[test]
fn should_match_php_stub_published_order_and_optional_extension() {
    let files = alef::backends::php::PhpBackend
        .generate_type_stubs(&api(true, true, false), &config())
        .unwrap();
    let source = files
        .into_iter()
        .map(|file| file.content)
        .collect::<Vec<_>>()
        .join("\n");
    let signature = regex::Regex::new(r"(?s)function __construct\((.*?)\)")
        .unwrap()
        .captures(&source)
        .unwrap()[1]
        .to_string();
    let names = regex::Regex::new(r"\$(\w+)")
        .unwrap()
        .captures_iter(&signature)
        .map(|captures| captures[1].to_string())
        .collect::<Vec<_>>();
    let mut expected = (0..12).map(|index| format!("flag{index}")).collect::<Vec<_>>();
    expected.extend(["maxDepth".into(), "keepHidden".into()]);
    assert_eq!(names, expected, "{signature}");
    assert!(signature.contains("?bool $keepHidden = null"), "{signature}");
}

#[test]
fn should_omit_binding_excluded_extensions_without_reordering_legacy_fields() {
    for backend in [
        &alef::backends::php::PhpBackend as &dyn Backend,
        &alef::backends::wasm::WasmBackend,
        &alef::backends::zig::ZigBackend,
    ] {
        let mut hidden = api(true, true, false);
        hidden.types[0]
            .fields
            .iter_mut()
            .find(|field| field.name == "keep_hidden")
            .unwrap()
            .binding_excluded = true;
        assert_eq!(render(backend, &hidden), render(backend, &api(true, false, false)));
    }
}

fn parse_constructor(source: &str) -> syn::ImplItemFn {
    let start = source.find("pub fn new(").unwrap();
    let opening = start + source[start..].find('{').unwrap();
    let mut depth = 0;
    let end = source[opening..]
        .char_indices()
        .find_map(|(index, character)| {
            match character {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(opening + index + 1)
        })
        .unwrap();
    syn::parse_str(&source[start..end]).unwrap()
}

fn constructor_program(function: &syn::ImplItemFn, default: bool) -> String {
    use quote::ToTokens;
    let parameters = function
        .sig
        .inputs
        .iter()
        .map(|argument| match argument {
            syn::FnArg::Typed(parameter) => parameter,
            _ => panic!("static constructor"),
        })
        .collect::<Vec<_>>();
    let max_type = parameters[12].ty.to_token_stream().to_string();
    let flags = (0..12).map(|index| format!("flag_{index}: bool,")).collect::<String>();
    let mut arguments = parameters[..12]
        .iter()
        .map(|parameter| {
            if parameter.ty.to_token_stream().to_string().starts_with("Option") {
                "None"
            } else {
                "false"
            }
        })
        .collect::<Vec<_>>();
    arguments.extend(["Some(31)", "None"]);
    let omitted = arguments.join(",");
    arguments[13] = "Some(true)";
    let explicit = arguments.join(",");
    format!(
        "#[derive(Default)] struct Options {{ {flags} max_depth: {max_type}, keep_hidden: bool }} type WasmOptions = Options; impl Options {{ {} }} fn main() {{ let old = Options::new({omitted}); assert_eq!(old.max_depth, Some(31)); assert_eq!(old.keep_hidden, {default}); let explicit = Options::new({explicit}); assert_eq!(explicit.max_depth, Some(31)); assert!(explicit.keep_hidden); }}",
        function.to_token_stream()
    )
}

fn compile_rust_program(program: &str) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("oracle.rs");
    std::fs::write(&path, program).unwrap();
    let binary = directory
        .path()
        .join(if cfg!(windows) { "oracle.exe" } else { "oracle" });
    let compilation = std::process::Command::new("rustc")
        .args(["--edition=2024", "-o"])
        .arg(&binary)
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        compilation.status.success(),
        "{}",
        String::from_utf8_lossy(&compilation.stderr)
    );
    let execution = std::process::Command::new(binary).output().unwrap();
    assert!(
        execution.status.success(),
        "{}",
        String::from_utf8_lossy(&execution.stderr)
    );
}

#[test]
fn should_execute_php_and_wasm_omission_explicit_true_and_legacy_max_depth() {
    for backend in [
        &alef::backends::php::PhpBackend as &dyn Backend,
        &alef::backends::wasm::WasmBackend,
    ] {
        for default in [false, true] {
            let source = render(backend, &api(true, true, default));
            compile_rust_program(&constructor_program(&parse_constructor(&source), default));
        }
    }
}

#[test]
#[ignore = "requires the Zig toolchain; run with --include-ignored"]
fn should_execute_zig_legacy_literal_and_explicit_extension() {
    let declaration_pattern = regex::Regex::new(r"(?s)pub const Options = struct \{.*?\};").unwrap();
    for default in [false, true] {
        let source = render(&alef::backends::zig::ZigBackend, &api(true, true, default));
        let declaration = declaration_pattern.find(&source).unwrap().as_str();
        let fields = (0..12)
            .map(|index| format!(".flag_{index} = false,"))
            .collect::<String>();
        let program = format!(
            r#"const std = @import("std");
{declaration}
test "legacy literal and explicit extension" {{
    const legacy: Options = .{{ {fields} .max_depth = 31 }};
    try std.testing.expect(legacy.max_depth.? == 31);
    try std.testing.expectEqual({default}, legacy.keep_hidden);
    const explicit: Options = .{{ {fields} .max_depth = 31, .keep_hidden = true }};
    try std.testing.expectEqual(true, explicit.keep_hidden);
}}
"#
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oracle.zig");
        std::fs::write(&path, program).unwrap();
        let execution = std::process::Command::new("zig")
            .arg("test")
            .arg(path)
            .output()
            .unwrap();
        assert!(
            execution.status.success(),
            "{}",
            String::from_utf8_lossy(&execution.stderr)
        );
    }
}

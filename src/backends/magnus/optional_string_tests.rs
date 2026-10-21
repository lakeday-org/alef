use crate::backends::magnus::MagnusBackend;
use crate::core::backend::Backend;
use crate::core::config::new_config::NewAlefConfig;
use crate::core::ir::{ApiSurface, FunctionDef, ParamDef, TypeRef};

fn generated_optional_string(is_async: bool) -> String {
    let config: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["ruby"]
[[crates]]
name = "sample-core"
sources = ["src/lib.rs"]
"#,
    )
    .unwrap();
    let api = ApiSurface {
        crate_name: "sample_core".to_string(),
        functions: vec![FunctionDef {
            name: "echo_optional".to_string(),
            rust_path: "sample_core::echo_optional".to_string(),
            params: vec![ParamDef {
                name: "value".to_string(),
                ty: TypeRef::String,
                optional: true,
                ..Default::default()
            }],
            return_type: TypeRef::Optional(Box::new(TypeRef::String)),
            is_async,
            ..Default::default()
        }],
        ..Default::default()
    };
    MagnusBackend
        .generate_bindings(&api, &config.resolve().unwrap().remove(0))
        .unwrap()
        .into_iter()
        .find(|file| file.path.ends_with("lib.rs"))
        .unwrap()
        .content
}

fn assert_optional_string_errors_propagate(is_async: bool) {
    let source = generated_optional_string(is_async);
    syn::parse_file(&source).expect("generated bindings must parse");
    assert!(source.contains("Option<magnus::Value>"), "must exercise scan_args");
    assert!(
        source.contains("String::try_convert"),
        "must exercise string conversion"
    );
    assert!(
        source.contains("Some(String::try_convert(v)?)"),
        "wrong-type optional strings must propagate Magnus conversion errors:\n{source}"
    );
    assert!(!source.contains("String::try_convert(v).unwrap_or_default()"));
}

#[test]
fn should_propagate_wrong_type_optional_string_in_sync_wrapper() {
    assert_optional_string_errors_propagate(false);
}

#[test]
fn should_propagate_wrong_type_optional_string_in_async_wrappers() {
    assert_optional_string_errors_propagate(true);
}

fn write_native_fixture(directory: &std::path::Path) {
    let source = generated_optional_string(false);
    let conversions: Vec<_> = source
        .lines()
        .filter(|line| line.contains("let value: Option<String>"))
        .collect();
    assert_eq!(conversions.len(), 1, "compile the actual emitted conversion");
    let wrapper = include_str!("../../../tests/fixtures/magnus_optional_string/wrapper.rs.txt");
    std::fs::create_dir(directory.join("src")).unwrap();
    std::fs::write(
        directory.join("src/lib.rs"),
        wrapper.replace("CONVERSION", conversions[0]),
    )
    .unwrap();
    std::fs::write(
        directory.join("Cargo.toml"),
        include_str!("../../../tests/fixtures/magnus_optional_string/Cargo.toml.txt"),
    )
    .unwrap();
    std::fs::write(
        directory.join("check.rb"),
        include_str!("../../../tests/fixtures/magnus_optional_string/check.rb"),
    )
    .unwrap();
    std::fs::write(
        directory.join("Taskfile.yml"),
        "version: '3'\ntasks:\n  build:\n    cmds: ['cargo build']\n",
    )
    .unwrap();
    if cfg!(target_os = "macos") {
        std::fs::write(
            directory.join("build.rs"),
            "fn main() { println!(\"cargo:rustc-link-arg=-Wl,-undefined,dynamic_lookup\"); }",
        )
        .unwrap();
    }
}

/// The Rust target whose ABI matches the Ruby the extension is loaded into, or `None` when the
/// host default target already matches.
///
/// `rb-sys` refuses to build against Ruby headers from a different operating-system ABI.
/// RubyInstaller (the Ruby on Windows runners) is MinGW, while the host Rust default there is
/// MSVC, so the extension must be built for the matching GNU target.
fn rust_target_for_ruby(target_os: &str, target_cpu: &str) -> Option<&'static str> {
    if !target_os.starts_with("mingw") {
        return None;
    }
    match target_cpu {
        "x64" | "x86_64" => Some("x86_64-pc-windows-gnu"),
        "i386" | "x86" => Some("i686-pc-windows-gnu"),
        _ => None,
    }
}

fn ruby_extension_target(ruby_binary: &std::path::Path) -> Option<&'static str> {
    let output = std::process::Command::new(ruby_binary)
        .args([
            "-rrbconfig",
            "-e",
            "print RbConfig::CONFIG['target_os'], ' ', RbConfig::CONFIG['target_cpu']",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let reported = String::from_utf8(output.stdout).unwrap();
    let (target_os, target_cpu) = reported.split_once(' ').expect("Ruby reports target_os and target_cpu");
    rust_target_for_ruby(target_os, target_cpu)
}

#[test]
fn should_build_for_the_rust_target_matching_the_ruby_abi() {
    assert_eq!(rust_target_for_ruby("mingw32", "x64"), Some("x86_64-pc-windows-gnu"));
    assert_eq!(rust_target_for_ruby("mingw-ucrt", "x64"), Some("x86_64-pc-windows-gnu"));
    assert_eq!(rust_target_for_ruby("mingw32", "i386"), Some("i686-pc-windows-gnu"));
    assert_eq!(rust_target_for_ruby("linux", "x86_64"), None);
    assert_eq!(rust_target_for_ruby("darwin23", "arm64"), None);
}

#[test]
fn should_report_the_target_of_the_installed_ruby() {
    let Some(ruby_binary) = crate::test_support::toolchain::RUBY.open() else {
        return;
    };
    let target = ruby_extension_target(&ruby_binary);
    assert_eq!(
        target.is_some(),
        cfg!(windows),
        "only RubyInstaller on Windows needs a GNU target"
    );
}

#[test]
fn should_raise_type_error_in_compiled_ruby_extension() {
    let Some(ruby_binary) = crate::test_support::toolchain::RUBY.open() else {
        return;
    };
    let fixture = tempfile::tempdir().unwrap();
    write_native_fixture(fixture.path());
    let rust_target = ruby_extension_target(&ruby_binary);
    let mut build = std::process::Command::new("task");
    build
        .args(["build"])
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_TARGET_DIR", fixture.path().join("target"))
        .current_dir(fixture.path());
    if let Some(target) = rust_target {
        build.env("CARGO_BUILD_TARGET", target);
    }
    let build = build.output().unwrap();
    assert!(build.status.success(), "{}", String::from_utf8_lossy(&build.stderr));
    let filename = if cfg!(target_os = "macos") {
        "liboptional_string_fixture.dylib"
    } else if cfg!(target_os = "windows") {
        "optional_string_fixture.dll"
    } else {
        "liboptional_string_fixture.so"
    };
    let extension_name = if cfg!(target_os = "macos") {
        "optional_string_fixture.bundle"
    } else {
        "optional_string_fixture.so"
    };
    let extension = fixture.path().join(extension_name);
    let mut artifact_dir = fixture.path().join("target");
    if let Some(target) = rust_target {
        artifact_dir.push(target);
    }
    std::fs::copy(artifact_dir.join("debug").join(filename), &extension).unwrap();
    let ruby = std::process::Command::new(ruby_binary)
        .arg(fixture.path().join("check.rb"))
        .arg(extension)
        .output()
        .unwrap();
    assert!(ruby.status.success(), "{}", String::from_utf8_lossy(&ruby.stderr));
    assert_eq!(
        String::from_utf8(ruby.stdout).unwrap().trim(),
        "PASS: 4 valid controls, 4 TypeError cases"
    );
}

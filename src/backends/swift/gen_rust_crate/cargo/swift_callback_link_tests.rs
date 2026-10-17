use super::emit_build_rs;
use crate::process::capture::{OUTPUT_DRAIN_GRACE, collect_output_within, output_reader};
use crate::process::timed::{Deadline, GroupChild};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const COMPILE_BUDGET: Duration = Duration::from_secs(120);
const PROBE_BUDGET: Duration = Duration::from_secs(5);
const EXPECTED_PROBES: [&str; 7] = [
    "macos-inbound",
    "ios",
    "linux",
    "windows",
    "feature-off",
    "feature-on",
    "no-inbound",
];
const EXPECTED_CALLBACKS: usize = 2;
const STRICT_FLAGS: &str = "-D warnings -A unpredictable-function-pointer-comparisons -A mismatched-lifetime-syntaxes";
const PACKAGE_NAME: &str = "callback-policy-probe";
const INBOUND: &str = r#"
#[swift_bridge::bridge]
mod ffi {
    extern "Swift" {
        type CounterBox;
        fn evaluate(&self, value: u32) -> u32;
    }
}
"#;
const NO_INBOUND: &str = r#"
#[swift_bridge::bridge]
mod ffi { extern "Rust" { fn evaluate(value: u32) -> u32; } }
"#;
const MANIFEST: &str = r#"
[package]
name = "callback-policy-probe"
version = "0.0.0"
edition = "2024"
build = false
[workspace]
[[bin]]
name = "build-policy"
path = "build-policy.rs"
[dependencies]
swift-bridge-build = "=0.1.59"
"#;

struct ChildGuard(Option<GroupChild>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            child.kill_tree();
        }
    }
}

struct Scratch(Option<tempfile::TempDir>);

impl Drop for Scratch {
    fn drop(&mut self) {
        let retain = std::thread::panicking()
            || std::env::var("ALEF_SWIFT_CALLBACK_TEST_KEEP_ARTIFACTS").ok().as_deref() == Some("1");
        if retain && let Some(directory) = self.0.take() {
            let retained = directory.keep();
            tracing::info!(?retained, "retained Swift callback-policy fixture");
        }
    }
}

struct ProbeCase<'a> {
    name: &'a str,
    os: &'a str,
    vendor: &'a str,
    callbacks: bool,
}

fn run_bounded(command: &mut Command, budget: Duration, evidence: &Path) -> String {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut guard = ChildGuard(Some(GroupChild::spawn(command).expect("spawn fixture child")));
    let child = guard.0.as_mut().expect("active child");
    let stdout = child.take_stdout().map(output_reader);
    let stderr = child.take_stderr().map(output_reader);
    let deadline = child.wait_within(budget, &command).expect("wait for fixture child");
    let output = collect_output_within(stdout, stderr, OUTPUT_DRAIN_GRACE).expect("drain fixture output");
    std::fs::write(evidence.with_extension("stdout"), &output.stdout).expect("save fixture stdout");
    std::fs::write(evidence.with_extension("stderr"), &output.stderr).expect("save fixture stderr");
    assert!(output.complete, "child output drain deadline exceeded");
    let Deadline::Exited(status) = deadline else {
        panic!("child deadline exceeded");
    };
    drop(guard.0.take());
    assert!(status.success(), "fixture infrastructure failed: {}", output.stderr);
    output.stdout
}

fn generated_cdecls(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in source.lines().map(str::trim).filter(|line| line.starts_with("@_cdecl")) {
        let name = line
            .strip_prefix("@_cdecl(\"")
            .and_then(|rest| rest.strip_suffix("\")"))
            .expect("unexpected swift-bridge cdecl syntax");
        assert!(!name.is_empty());
        assert!(
            !name.chars().any(|character| {
                character.is_control() || character.is_whitespace() || matches!(character, ',' | '"' | '\\')
            }),
            "unexpected quoting, escaping or separators in fixture export"
        );
        assert!(names.insert(name.to_owned()), "duplicate actual Swift export");
    }
    names
}

fn callback_arguments(stdout: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in stdout.lines().filter(|line| line.contains("rustc-cdylib-link-arg")) {
        let name = line
            .strip_prefix("cargo:rustc-cdylib-link-arg=-Wl,-U,_")
            .expect("unexpected cdylib-link directive");
        assert!(names.insert(name.to_owned()), "duplicate callback allowlist directive");
    }
    names
}

fn prepare_fixture(root: &Path) {
    std::fs::create_dir(root.join("src")).expect("create fixture source directory");
    std::fs::write(root.join("build-policy.rs"), emit_build_rs()).expect("write actual emitted build script");
    std::fs::write(root.join("Cargo.toml"), MANIFEST).expect("write fixture manifest");
}

fn compile_probe(root: &Path) -> PathBuf {
    let mut cargo = Command::new(env!("CARGO"));
    cargo
        .current_dir(root)
        .args(["build", "--bin", "build-policy"])
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("RUSTFLAGS", STRICT_FLAGS)
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_BUILD_TARGET");
    run_bounded(&mut cargo, COMPILE_BUDGET, &root.join("compile-build-policy"));
    root.join(format!("target/debug/build-policy{}", std::env::consts::EXE_SUFFIX))
}

fn isolate_target(command: &mut Command, case: &ProbeCase<'_>) {
    for (key, _) in std::env::vars_os() {
        let key_name = key.to_string_lossy();
        if key_name.starts_with("CARGO_FEATURE_") || key_name.starts_with("CARGO_CFG_") {
            command.env_remove(key);
        }
    }
    command
        .env("CARGO_CFG_TARGET_OS", case.os)
        .env("CARGO_CFG_TARGET_VENDOR", case.vendor)
        .env(
            "CARGO_CFG_TARGET_FAMILY",
            if case.os == "windows" { "windows" } else { "unix" },
        )
        .env(
            "CARGO_CFG_TARGET_ARCH",
            if case.os == "windows" { "x86_64" } else { "aarch64" },
        );
    if case.callbacks {
        command.env("CARGO_FEATURE_CALLBACKS", "1");
    }
}

fn record_probe(root: &Path, name: &str) {
    let mut ledger = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("probe-ledger.txt"))
        .expect("open probe ledger");
    writeln!(ledger, "{name}").expect("record completed probe");
}

fn run_probe(executable: &Path, root: &Path, case: &ProbeCase<'_>) -> String {
    let out = root.join(format!("out-{}", case.name));
    std::fs::create_dir(&out).expect("create unique probe output directory");
    let mut command = Command::new(executable);
    isolate_target(&mut command, case);
    command
        .current_dir(root)
        .env("OUT_DIR", &out)
        .env("CARGO_PKG_NAME", PACKAGE_NAME)
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER");
    let stdout = run_bounded(&mut command, PROBE_BUDGET, &root.join(case.name));
    record_probe(root, case.name);
    stdout
}

fn swift_exports(root: &Path, case: &str) -> BTreeSet<String> {
    let source = root.join(format!("out-{case}/{PACKAGE_NAME}/{PACKAGE_NAME}.swift"));
    generated_cdecls(&std::fs::read_to_string(source).expect("read actual generated Swift"))
}

fn assert_initial_apple_policy(executable: &Path, root: &Path) -> BTreeSet<String> {
    std::fs::write(root.join("src/lib.rs"), INBOUND).expect("write inbound bridge");
    let case = ProbeCase {
        name: "macos-inbound",
        os: "macos",
        vendor: "apple",
        callbacks: false,
    };
    let stdout = run_probe(executable, root, &case);
    let expected = swift_exports(root, case.name);
    assert_eq!(
        expected.len(),
        EXPECTED_CALLBACKS,
        "one actual method plus one ARC destructor"
    );
    let actual = callback_arguments(&stdout);
    assert_eq!(
        actual.len(),
        EXPECTED_CALLBACKS,
        "expected two actual callback allowances"
    );
    assert_eq!(actual, expected);
    assert!(
        !stdout.contains("cargo:rustc-link-arg="),
        "no directive applies to every artifact"
    );
    assert!(!stdout.contains("-undefined"), "no global unresolved-symbol allowance");
    expected
}

fn assert_target_policies(executable: &Path, root: &Path, expected: &BTreeSet<String>) {
    for (os, vendor) in [("ios", "apple"), ("linux", "unknown"), ("windows", "pc")] {
        let case = ProbeCase {
            name: os,
            os,
            vendor,
            callbacks: false,
        };
        let actual = callback_arguments(&run_probe(executable, root, &case));
        let wanted = if vendor == "apple" {
            expected.clone()
        } else {
            BTreeSet::new()
        };
        assert_eq!(actual, wanted, "target {os}");
    }
}

fn assert_feature_policies(executable: &Path, root: &Path, expected: &BTreeSet<String>) {
    let gated = INBOUND.replacen("mod ffi", "#[cfg(feature = \"callbacks\")]\nmod ffi", 1);
    std::fs::write(root.join("src/lib.rs"), gated).expect("write feature-gated bridge");
    for (name, callbacks) in [("feature-off", false), ("feature-on", true)] {
        let case = ProbeCase {
            name,
            os: "macos",
            vendor: "apple",
            callbacks,
        };
        let wanted = if callbacks { expected.clone() } else { BTreeSet::new() };
        assert_eq!(callback_arguments(&run_probe(executable, root, &case)), wanted);
        assert_eq!(swift_exports(root, case.name), wanted);
    }
}

fn assert_no_inbound_policy(executable: &Path, root: &Path) {
    std::fs::write(root.join("src/lib.rs"), NO_INBOUND).expect("write Rust-only bridge");
    let case = ProbeCase {
        name: "no-inbound",
        os: "macos",
        vendor: "apple",
        callbacks: false,
    };
    assert_eq!(callback_arguments(&run_probe(executable, root, &case)), BTreeSet::new());
    assert_eq!(swift_exports(root, case.name), BTreeSet::new());
}

#[test]
#[ignore = "compiles emitted Swift build.rs; run explicitly with an offline cache and compiler lease"]
fn should_allow_only_actual_swift_callbacks_in_apple_cdylibs() {
    let scratch = Scratch(Some(tempfile::tempdir().expect("create callback-policy fixture")));
    let root = scratch.0.as_ref().expect("active fixture").path();
    prepare_fixture(root);
    let executable = compile_probe(root);
    let expected = assert_initial_apple_policy(&executable, root);
    assert_target_policies(&executable, root, &expected);
    assert_feature_policies(&executable, root, &expected);
    assert_no_inbound_policy(&executable, root);
    let ledger = std::fs::read_to_string(root.join("probe-ledger.txt")).expect("read completed probe ledger");
    assert_eq!(ledger.lines().collect::<Vec<_>>(), EXPECTED_PROBES);
}

const EXPECTED_HELPER_CASES: usize = 10;
const EXPORT_HELPER_TESTS: &str = r#"
#[cfg(test)]
mod callback_export_tests {
    use super::swift_callback_exports;

    #[test]
    fn should_preserve_unicode_exports_in_sorted_order() {
        let source = "@_cdecl(\"z\")\n@_cdecl(\"méthode\")\n@_cdecl(\"a\")";
        let actual = swift_callback_exports(source).into_iter().collect::<Vec<_>>();
        assert_eq!(actual, ["a", "méthode", "z"]);
    }

    #[test]
    fn should_return_no_exports_for_swift_without_callbacks() {
        assert!(swift_callback_exports("func ordinary() {}\n").is_empty());
    }

    #[test]
    #[should_panic(expected = "malformed swift-bridge @_cdecl export")]
    fn should_reject_malformed_export_syntax() {
        swift_callback_exports("@_cdecl(\"missing-quote)");
    }

    #[test]
    #[should_panic(expected = "invalid Swift callback export symbol")]
    fn should_reject_empty_export_name() {
        swift_callback_exports("@_cdecl(\"\")");
    }

    #[test]
    #[should_panic(expected = "invalid Swift callback export symbol")]
    fn should_reject_comma_linker_argument_separator() {
        swift_callback_exports("@_cdecl(\"first,second\")");
    }

    #[test]
    #[should_panic(expected = "invalid Swift callback export symbol")]
    fn should_reject_whitespace_in_export_name() {
        swift_callback_exports("@_cdecl(\"first second\")");
    }

    #[test]
    #[should_panic(expected = "invalid Swift callback export symbol")]
    fn should_reject_control_character_in_export_name() {
        swift_callback_exports("@_cdecl(\"first\0second\")");
    }

    #[test]
    #[should_panic(expected = "invalid Swift callback export symbol")]
    fn should_reject_embedded_quote_in_export_name() {
        swift_callback_exports("@_cdecl(\"first\"second\")");
    }

    #[test]
    #[should_panic(expected = "invalid Swift callback export symbol")]
    fn should_reject_unsupported_swift_literal_escape() {
        swift_callback_exports("@_cdecl(\"first\\second\")");
    }

    #[test]
    #[should_panic(expected = "duplicate Swift callback export symbol")]
    fn should_reject_duplicate_exports() {
        swift_callback_exports("@_cdecl(\"same\")\n@_cdecl(\"same\")");
    }
}
"#;

fn append_export_helper_tests(root: &Path) {
    let path = root.join("build-policy.rs");
    let emitted = std::fs::read_to_string(&path).expect("read exact emitted build script");
    std::fs::write(&path, format!("{emitted}{EXPORT_HELPER_TESTS}")).expect("append private fixture tests");
    let actual = std::fs::read_to_string(path).expect("read actual fixture test source");
    assert_eq!(actual.strip_suffix(EXPORT_HELPER_TESTS), Some(emitted.as_str()));
    assert_eq!(emitted, emit_build_rs());
}

fn run_export_helper_tests(root: &Path) -> String {
    let mut cargo = Command::new(env!("CARGO"));
    cargo
        .current_dir(root)
        .args(["test", "--bin", "build-policy", "--", "--test-threads=1"])
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_PROFILE_TEST_DEBUG", "0")
        .env("RUSTFLAGS", STRICT_FLAGS)
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_BUILD_TARGET");
    run_bounded(&mut cargo, COMPILE_BUDGET, &root.join("export-helper-tests"))
}

#[test]
#[ignore = "compiles actual emitted export helper; run explicitly with an offline cache and compiler lease"]
fn should_validate_actual_emitted_swift_callback_exports() {
    let scratch = Scratch(Some(tempfile::tempdir().expect("create export-helper fixture")));
    let root = scratch.0.as_ref().expect("active fixture").path();
    prepare_fixture(root);
    append_export_helper_tests(root);
    let stdout = run_export_helper_tests(root);
    let expected = format!("test result: ok. {EXPECTED_HELPER_CASES} passed; 0 failed; 0 ignored;");
    assert!(
        stdout.contains(&expected),
        "missing nonempty exact helper-test census: {stdout}"
    );
}

const PRE_TEMPLATE_BUILD_SCRIPT: &str = r##"// Generated by alef. Do not edit by hand.
use std::collections::BTreeSet;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR unset"));
    let crate_name = std::env::var("CARGO_PKG_NAME").expect("CARGO_PKG_NAME unset");
    let bridges = vec!["src/lib.rs"];
    let generated = swift_bridge_build::parse_bridges(bridges);
    if std::env::var("CARGO_CFG_TARGET_VENDOR").as_deref() == Ok("apple") {
        let swift = generated.concat_swift();
        // Swift owns these exports, which are linked after Cargo builds the cdylib. ~keep
        for name in swift_callback_exports(&swift) {
            println!("cargo:rustc-cdylib-link-arg=-Wl,-U,_{name}");
        }
    }
    generated.write_all_concatenated(out_dir, &crate_name);
    println!("cargo:rerun-if-changed=src/lib.rs");
}

fn swift_callback_exports(swift: &str) -> BTreeSet<&str> {
    let mut names = BTreeSet::new();
    for line in swift.lines().map(str::trim).filter(|line| line.starts_with("@_cdecl")) {
        let name = line
            .strip_prefix("@_cdecl(\"")
            .and_then(|rest| rest.strip_suffix("\")"))
            .expect("malformed swift-bridge @_cdecl export");
        assert!(
            !name.is_empty()
                && !name.chars().any(|character| {
                    character.is_control() || character.is_whitespace() || matches!(character, ',' | '"' | '\\')
                }),
            "invalid Swift callback export symbol"
        );
        assert!(names.insert(name), "duplicate Swift callback export symbol");
    }
    names
}
"##;

#[test]
fn should_preserve_registered_build_script_after_template_migration() {
    let expected = crate::core::keep_marker::strip_keep_markers(PRE_TEMPLATE_BUILD_SCRIPT);
    assert_eq!(emit_build_rs(), expected);
}

use super::render_raise_error_clause;
use super::tests::{errors, errors_with_hidden, fixture};
use crate::core::ir::ApiSurface;
use std::path::Path;
use std::process::{Command, Output};

fn task(directory: &Path, target: &str) -> Output {
    task_args(directory, target, &[])
}

fn task_args(directory: &Path, target: &str, arguments: &[&str]) -> Output {
    Command::new(executable("task", directory))
        .arg(target)
        .arg("--")
        .args(arguments)
        .current_dir(directory)
        .env("CARGO_TARGET_DIR", directory.join("target"))
        .env("RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", "")
        .output()
        .expect("run owned native fixture task")
}

fn render_cases(cases: &[(&str, &str, String, bool)]) -> String {
    let cases: Vec<_> = cases
        .iter()
        .map(|(name, call, matcher, rejected)| {
            crate::alef_context! { name => *name, call => *call, matcher => matcher, rejected => *rejected }
        })
        .collect();
    crate::e2e::template_env::render(
        "ruby/error_matcher_runtime.rb.jinja",
        crate::alef_context! { cases => cases },
    )
}

#[test]
#[ignore = "requires Ruby headers, Magnus and RSpec"]
fn generated_matchers_accept_native_typed_errors_and_reject_old_runtime_matcher() {
    let directory = tempfile::tempdir().expect("owned native fixture");
    let path = directory.path();
    let errors = errors();
    prepare_native(path, &errors);
    let cases = positive_cases(&errors);
    std::fs::write(path.join("matcher_spec.rb"), render_cases(&cases)).expect("positive spec");
    let output = task(path, "test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("8 examples, 0 failures"), "{stdout}");
    let old = [(
        "old matcher",
        "Sample.bad_request",
        "raise_error(RuntimeError)".into(),
        false,
    )];
    std::fs::write(path.join("matcher_spec.rb"), render_cases(&old)).expect("negative spec");
    let output = task(path, "test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "old RuntimeError matcher unexpectedly accepted typed error"
    );
    assert!(stdout.contains("1 example, 1 failure"), "{stdout}");
    assert!(
        stdout.contains("expected RuntimeError") && stdout.contains("Sample::BadRequestError"),
        "{stdout}"
    );
}

fn install_extension(path: &Path) {
    let output = Command::new(executable("ruby", path))
        .args(["-rrbconfig", "-e", "print RbConfig::CONFIG.fetch('DLEXT')"])
        .current_dir(path)
        .output()
        .expect("Ruby extension suffix");
    assert!(output.status.success());
    let extension = String::from_utf8(output.stdout).expect("extension suffix");
    assert!(!extension.is_empty());
    let library = if cfg!(target_os = "macos") {
        "libruby_error_matcher_fixture.dylib"
    } else if cfg!(windows) {
        "ruby_error_matcher_fixture.dll"
    } else {
        "libruby_error_matcher_fixture.so"
    };
    std::fs::copy(
        path.join("target/debug").join(library),
        path.join(format!("ruby_error_matcher_fixture.{extension}")),
    )
    .expect("install owned extension");
}

fn prepare_native(path: &Path, errors: &[crate::core::ir::ErrorDef]) {
    std::fs::create_dir(path.join("src")).expect("source directory");
    std::fs::write(
        path.join("Cargo.toml"),
        include_str!("../../../../../tests/fixtures/ruby_error_matcher/Cargo.toml.txt"),
    )
    .expect("manifest");
    std::fs::write(
        path.join("Taskfile.yml"),
        include_str!("../../../../../tests/fixtures/ruby_error_matcher/Taskfile.yml"),
    )
    .expect("tasks");
    let api = crate::codegen::binding_projection::project_owned(ApiSurface {
        errors: errors.to_vec(),
        ..Default::default()
    });
    let source = crate::e2e::template_env::render(
        "ruby/error_matcher_native.rs.jinja",
        crate::alef_context! {
            converter => crate::codegen::error_gen::gen_magnus_error_converter(&errors[1], "sample", "Sample"),
            registrations => crate::codegen::error_gen::gen_magnus_error_class_registrations(&api).expect("registrations"),
        },
    );
    std::fs::write(path.join("src/lib.rs"), source).expect("native source");
    let output = task(path, "build");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    install_extension(path);
}

fn executable(name: &str, directory: &Path) -> std::path::PathBuf {
    which::which_in(name, std::env::var_os("PATH"), directory).expect("native fixture executable")
}

type NativeCase = (&'static str, &'static str, String, bool);

fn positive_cases(errors: &[crate::core::ir::ErrorDef]) -> [NativeCase; 8] {
    let matcher = |value| render_raise_error_clause(&fixture(value), errors, "Sample");
    [
        (
            "typed variant",
            "Sample.bad_request",
            matcher(Some("BadRequest")),
            false,
        ),
        (
            "builtin collision",
            "Sample.connection",
            matcher(Some("Connection")),
            false,
        ),
        ("bare typed", "Sample.bad_request", matcher(None), false),
        ("base fallback", "Sample.base_error", matcher(None), false),
        ("native fallback", "Sample.runtime_error", matcher(None), false),
        (
            "literal message",
            "Sample.bad_request",
            matcher(Some("missing.field[0]")),
            false,
        ),
        (
            "wrong variant rejected",
            "Sample.connection",
            matcher(Some("BadRequest")),
            true,
        ),
        (
            "unrelated errors rejected",
            "raise ArgumentError, 'unrelated'",
            matcher(None),
            true,
        ),
    ]
}

#[test]
#[ignore = "requires Ruby headers, Magnus and RSpec"]
fn skipped_error_matchers_reject_the_missing_constant_negative_control() {
    let directory = tempfile::tempdir().expect("owned native fixture");
    let path = directory.path();
    let errors = errors_with_hidden();
    prepare_native(path, &errors);
    let mut unprojected = errors.clone();
    unprojected.last_mut().expect("Rust-only error").binding_excluded = false;
    let old = [(
        "unprojected matcher",
        "Sample.bad_request",
        render_raise_error_clause(&fixture(None), &unprojected, "Sample"),
        false,
    )];
    std::fs::write(path.join("matcher_spec.rb"), render_cases(&old)).expect("negative spec");
    let output = task_args(path, "test", &["--format", "json"]);
    assert_missing_constant(path, &output);
    let cases = positive_cases(&errors);
    std::fs::write(path.join("matcher_spec.rb"), render_cases(&cases)).expect("fixed spec");
    let output = task(path, "test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("8 examples, 0 failures"), "{stdout}");
}

fn assert_missing_constant(path: &Path, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success(), "missing constant control unexpectedly passed");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("RSpec JSON report");
    assert_eq!(report["summary"]["example_count"], 1, "{stdout}");
    assert_eq!(report["summary"]["failure_count"], 1, "{stdout}");
    assert_eq!(
        report["examples"][0]["exception"]["class"], "RSpec::Expectations::ExpectationNotMetError",
        "{stdout}"
    );
    let raised: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path.join("raised_errors.json")).expect("actual Ruby raises"))
            .expect("raised error report");
    assert!(
        raised
            .as_array()
            .expect("raised errors")
            .iter()
            .any(|error| error["class"] == "NameError"
                && error["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("Sample::HiddenCompileError"))),
        "{raised}"
    );
    assert!(
        report["examples"][0]["exception"]["message"]
            .as_str()
            .expect("exception message")
            .contains("Sample::HiddenCompileError"),
        "{stdout}"
    );
}

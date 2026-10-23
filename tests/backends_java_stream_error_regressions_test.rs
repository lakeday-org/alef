#![allow(clippy::print_stdout)]

//! Runtime proof for the generated Java stream iterator. The generated `watch` method runs
//! against a scripted stand-in for the native library, so the errors the native layer reports in
//! the middle of a stream reach the same code the shipped binding runs.

#[path = "backends_java_blocker_regressions/support.rs"]
mod support;

use alef::backends::java::JavaBackend;
use alef::core::backend::Backend;
use alef::core::config::NewAlefConfig;
use alef::core::ir::{ApiSurface, MethodDef, ReceiverKind, TypeDef, TypeRef};
use support::{compile_java, extract_java_method, java_available, run_java_args, write_file};

const METHOD_SIGNATURE: &str = "public java.util.stream.Stream<StreamItem> watch(";

fn generated_files() -> Vec<alef::core::GeneratedFile> {
    let config: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["java", "ffi"]

[[crates]]
name = "test_lib"
sources = ["src/lib.rs"]

[crates.ffi]
prefix = "tl"

[crates.java]
package = "com.test"

[[crates.adapters]]
name = "watch"
pattern = "streaming"
core_path = "watch"
owner_type = "SampleClient"
item_type = "StreamItem"
error_type = "TestError"
request_type = "test_lib::StreamRequest"

[[crates.adapters.params]]
name = "req"
type = "StreamRequest"
"#,
    )
    .expect("valid streaming config");
    let config = config.resolve().expect("resolved streaming config").remove(0);
    let serde_type = |name: &str, is_return_type: bool| TypeDef {
        name: name.into(),
        rust_path: format!("test_lib::{name}"),
        is_clone: true,
        has_default: true,
        has_serde: true,
        is_return_type,
        ..Default::default()
    };
    let api = ApiSurface {
        crate_name: "test_lib".into(),
        version: "0.1.0".into(),
        types: vec![
            TypeDef {
                name: "SampleClient".into(),
                rust_path: "test_lib::SampleClient".into(),
                is_opaque: true,
                methods: vec![MethodDef {
                    name: "watch".into(),
                    return_type: TypeRef::Unit,
                    is_async: true,
                    error_type: Some("TestError".into()),
                    receiver: Some(ReceiverKind::Ref),
                    ..Default::default()
                }],
                ..Default::default()
            },
            serde_type("StreamRequest", false),
            serde_type("StreamItem", true),
        ],
        ..Default::default()
    };
    JavaBackend
        .generate_bindings(&api, &config)
        .expect("Java streaming generation")
}

fn generated_client() -> String {
    generated_files()
        .into_iter()
        .find(|file| file.path.to_string_lossy().ends_with("SampleClient.java"))
        .expect("generated SampleClient.java")
        .content
}

fn run_scenario(scenario: &str) {
    if !java_available() {
        return;
    }
    let client = generated_client();
    let method = extract_java_method(&client, METHOD_SIGNATURE);
    let iterator = extract_java_method(&client, "private static final class WatchStreamingIterator");
    let implementation = [method.as_str(), iterator.as_str()].join("\n");
    assert!(
        method.contains("throws TestLibRsException"),
        "the harness stands in for TestLibRsException:\n{method}"
    );
    let directory = tempfile::tempdir().expect("temporary stream directory");
    write_file(
        directory.path(),
        "com/test/StreamHarness.java",
        &include_str!("fixtures/java_stream_error_harness.java").replace("GENERATED_METHOD", &implementation),
    );
    write_file(
        directory.path(),
        "com/test/NativeLib.java",
        include_str!("fixtures/java_stream_error_native_lib.java"),
    );
    write_file(
        directory.path(),
        "com/test/Types.java",
        include_str!("fixtures/java_stream_error_types.java"),
    );
    write_file(
        directory.path(),
        "com/test/StreamErrorMain.java",
        include_str!("fixtures/java_stream_error_main.java"),
    );
    compile_java(
        directory.path(),
        &[
            "com/test/Types.java",
            "com/test/NativeLib.java",
            "com/test/StreamHarness.java",
            "com/test/StreamErrorMain.java",
        ],
    );
    run_java_args(directory.path(), &["-cp", ".", "com.test.StreamErrorMain", scenario]);
}

#[test]
fn the_chunk_before_a_stream_error_is_returned_and_the_error_follows() {
    run_scenario("error_after_two_chunks");
}

#[test]
fn the_chunk_before_a_stream_error_is_returned_through_the_stream_api() {
    run_scenario("error_through_stream_api");
}

#[test]
fn a_stream_error_is_thrown_as_the_typed_binding_exception() {
    run_scenario("error_on_first_read");
}

#[test]
fn a_stream_rejected_on_open_is_thrown_as_the_typed_binding_exception() {
    run_scenario("stream_rejected_on_open");
}

#[test]
fn a_stream_that_ends_cleanly_delivers_every_chunk() {
    run_scenario("clean_end");
}

#[test]
#[ignore = "requires Maven and Checkstyle 14.3.0"]
fn generated_stream_methods_pass_checkstyle_method_length() {
    let directory = tempfile::tempdir().expect("owned Checkstyle fixture");
    let root = directory.path();
    write_style_fixture(root);
    reject_old_method(root);
    verify_generated_files(root);
}

fn write_style_fixture(root: &std::path::Path) {
    for (name, source) in [
        (
            "Taskfile.yml",
            include_str!("fixtures/java_stream_method_length/Taskfile.yml"),
        ),
        ("pom.xml", include_str!("fixtures/java_stream_method_length/pom.xml")),
        (
            "checkstyle.xml",
            include_str!("fixtures/java_stream_method_length/checkstyle.xml"),
        ),
    ] {
        write_file(root, name, source);
    }
}

fn reject_old_method(root: &std::path::Path) {
    write_file(
        root,
        "src/SampleClient.java",
        include_str!("fixtures/java_stream_method_length/baseline.java"),
    );
    let baseline = checkstyle(root);
    assert!(!baseline.success, "the old local iterator must fail MethodLength");
    let report = std::fs::read_to_string(root.join("target/checkstyle-result.xml")).unwrap();
    assert_eq!(report.matches("<file ").count(), 1, "{report}");
    assert_eq!(report.matches("<error ").count(), 1, "{report}");
    assert!(report.contains("MethodLengthCheck"), "{report}");
    assert!(report.contains("max allowed is 150"), "{report}");
}

fn verify_generated_files(root: &std::path::Path) {
    std::fs::remove_file(root.join("src/SampleClient.java")).unwrap();
    let files: Vec<_> = generated_files()
        .into_iter()
        .filter(|file| file.path.extension().is_some_and(|ext| ext == "java"))
        .collect();
    assert!(files.len() >= 5, "compile the actual DTO and native infrastructure");
    for file in &files {
        write_file(
            root,
            &format!("src/{}", file.path.file_name().unwrap().to_string_lossy()),
            &file.content,
        );
    }
    let fixed = checkstyle(root);
    let report = std::fs::read_to_string(root.join("target/checkstyle-result.xml")).unwrap();
    assert!(fixed.success, "{}\n{}\n{report}", fixed.stdout, fixed.stderr);
    assert_eq!(report.matches("<file ").count(), files.len(), "{report}");
    assert_eq!(report.matches("<error ").count(), 0, "{report}");
    let compiled = fixture_task(root, "compile");
    assert!(compiled.success, "{}\n{}", compiled.stdout, compiled.stderr);
    assert!(root.join("target/classes/com/test/SampleClient.class").is_file());
    assert!(
        root.join("target/classes/com/test/SampleClient$WatchStreamingIterator.class")
            .is_file()
    );
    println!("java_stream_generated_files={}", files.len());
}

fn checkstyle(root: &std::path::Path) -> alef::snippets::validators::CapturedStreams {
    fixture_task(root, "checkstyle")
}

fn fixture_task(root: &std::path::Path, target: &str) -> alef::snippets::validators::CapturedStreams {
    let executable = which::which_in("task", std::env::var_os("PATH"), root).expect("Task executable");
    let mut command = std::process::Command::new(executable);
    command.arg(target).current_dir(root);
    alef::snippets::validators::run_command_streams(&mut command, 600).expect("run actual Checkstyle")
}

#[test]
fn explicitly_closing_a_stream_frees_its_handle_once() {
    run_scenario("explicit_close");
}

#[test]
fn generated_stream_method_length_stays_within_the_checkstyle_limit() {
    const MAX_LINES: usize = 150;
    let baseline = extract_java_method(
        include_str!("fixtures/java_stream_method_length/baseline.java"),
        METHOD_SIGNATURE,
    );
    assert!(
        baseline.lines().count() > MAX_LINES,
        "the baseline must exercise the length limit"
    );
    let client = generated_client();
    let method = extract_java_method(&client, METHOD_SIGNATURE);
    assert!(
        method.lines().count() <= MAX_LINES,
        "generated method has {} lines",
        method.lines().count()
    );
    assert!(!method.contains("class WatchStreamingIterator"));
    let method_end = client.find(METHOD_SIGNATURE).unwrap() + method.len();
    let helper_start = client
        .find("private static final class WatchStreamingIterator")
        .expect("private helper");
    assert!(
        helper_start > method_end,
        "iterator class must be outside the public method"
    );
    println!(
        "java_stream_method_lines={} baseline_lines={} limit={MAX_LINES}",
        method.lines().count(),
        baseline.lines().count()
    );
}

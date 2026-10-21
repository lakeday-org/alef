//! A streaming call that fails after its first chunks answers the call with success. The error
//! arrives only while the stream is read. A generated test for a streaming fixture that expects
//! an error has to read the stream to its end, or a stream that ends quietly passes.
//!
//! Each test renders a fixture with a streaming call and an `error` assertion, then checks that
//! the read sits inside the same guarded region as the error check. A control fixture with a
//! call that is not a stream must keep its plain shape, so a fix that drains every error test
//! fails the control.

use alef::core::config::NewAlefConfig;
use alef::e2e::codegen::E2eCodegen;
use alef::e2e::codegen::elixir::ElixirCodegen;
use alef::e2e::codegen::go::GoCodegen;
use alef::e2e::codegen::java::JavaCodegen;
use alef::e2e::codegen::kotlin::KotlinE2eCodegen;
use alef::e2e::codegen::php::PhpCodegen;
use alef::e2e::codegen::rust::RustE2eCodegen;
use alef::e2e::codegen::swift::SwiftE2eCodegen;
use alef::e2e::fixture::{Assertion, Fixture, FixtureGroup, MockResponse};
use std::collections::BTreeMap;

const STREAM_FIXTURE: &str = "stream_fails_midway";
const PLAIN_FIXTURE: &str = "plain_call_fails";

fn toml_for(language: &str, language_block: &str) -> String {
    format!(
        r#"
[workspace]
languages = ["{language}"]

[[crates]]
name = "mylib"
sources = ["src/lib.rs"]

{language_block}

[crates.e2e]
fixtures = "fixtures"
output = "e2e"

[crates.e2e.call]
function = "plain_call"
module = "MyLib"
result_var = "result"
async = true
returns_result = true
streaming = false
args = []

[crates.e2e.calls.stream_call]
function = "stream_call"
module = "MyLib"
result_var = "result"
async = true
returns_result = true
streaming = true
args = []

[crates.e2e.calls.plain_call]
function = "plain_call"
module = "MyLib"
result_var = "result"
async = true
returns_result = true
streaming = false
args = []
"#
    )
}

fn error_assertion(value: Option<&str>) -> Assertion {
    Assertion {
        assertion_type: "error".to_string(),
        value: value.map(|v| serde_json::Value::String(v.to_string())),
        ..Assertion::default()
    }
}

/// The stream sends two chunks and then an error event after a 200 status.
fn stream_fixture(id: &str, call: &str, value: Option<&str>) -> Fixture {
    let chunk = serde_json::json!({"id": "c1", "choices": [{"delta": {"content": "hi"}}]});
    let failure = serde_json::json!({"error": {"message": "Rate limit reached", "type": "rate_limit_error"}});
    Fixture {
        id: id.to_string(),
        category: Some("streaming".to_string()),
        description: "stream fails after its first chunks".to_string(),
        call: Some(call.to_string()),
        input: serde_json::json!({}),
        mock_response: Some(MockResponse {
            status: 200,
            body: None,
            stream_chunks: Some(vec![chunk.clone(), chunk, failure]),
            headers: BTreeMap::new(),
        }),
        assertions: vec![error_assertion(value)],
        source: format!("streaming/{id}.json"),
        ..Fixture::default()
    }
}

fn render<C: E2eCodegen>(
    codegen: &C,
    toml_src: &str,
    value: Option<&str>,
    file_matches: fn(&str) -> bool,
) -> (String, String) {
    let cfg: NewAlefConfig = toml::from_str(toml_src).expect("config parses");
    let e2e = cfg.crates[0].e2e.clone().expect("e2e config present");
    let resolved = cfg.resolve().expect("config resolves").remove(0);
    let groups = vec![FixtureGroup {
        category: "streaming".to_string(),
        fixtures: vec![
            stream_fixture(STREAM_FIXTURE, "stream_call", value),
            stream_fixture(PLAIN_FIXTURE, "plain_call", value),
        ],
    }];
    let files = codegen
        .generate(&groups, &e2e, &resolved, &[], &[], &[], &[])
        .expect("generation succeeds");
    let paths: Vec<String> = files.iter().map(|f| f.path.display().to_string()).collect();
    let file = files
        .iter()
        .find(|f| {
            file_matches(&f.path.to_string_lossy()) && f.content.contains("Midway") | f.content.contains("midway")
        })
        .unwrap_or_else(|| panic!("no test file was emitted. Files: {paths:?}"));
    split_by_fixture(&file.content)
}

/// Cut one generated file into the stream fixture's test and the control fixture's test.
/// The two fixture ids never share a prefix, so the later id marks the cut.
fn split_by_fixture(content: &str) -> (String, String) {
    let stream_at = marker_index(content, STREAM_FIXTURE);
    let plain_at = marker_index(content, PLAIN_FIXTURE);
    if stream_at < plain_at {
        (
            content[stream_at..plain_at].to_string(),
            content[plain_at..].to_string(),
        )
    } else {
        (
            content[stream_at..].to_string(),
            content[plain_at..stream_at].to_string(),
        )
    }
}

/// The first place the fixture id appears in a form that names a test: snake, camel or Pascal.
fn marker_index(content: &str, id: &str) -> usize {
    let pascal: String = id
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect();
    [id.to_string(), pascal]
        .iter()
        .filter_map(|needle| content.find(needle.as_str()))
        .min()
        .unwrap_or_else(|| panic!("fixture `{id}` has no test in:\n{content}"))
}

fn position(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("`{needle}` is missing from:\n{haystack}"))
}

// ---- Rust ----

#[test]
fn rust_reads_the_stream_before_it_judges_the_call() {
    let toml_src = toml_for("rust", "");
    let (stream_test, plain_test) = render(&RustE2eCodegen, &toml_src, None, |p| p.ends_with(".rs"));

    let read = position(&stream_test, "tokio_stream::StreamExt::collect");
    let judge = position(&stream_test, "result.is_err()");
    assert!(
        read < judge,
        "the stream must be read before `is_err` runs:\n{stream_test}"
    );
    assert!(
        stream_test.contains("__item.err()"),
        "an item error must become the error the test judges:\n{stream_test}"
    );
    assert!(
        !plain_test.contains("tokio_stream"),
        "a call that is not a stream keeps its plain shape:\n{plain_test}"
    );
}

// ---- Go ----

#[test]
fn go_reads_the_stream_before_it_judges_the_call() {
    let toml_src = toml_for("go", "[crates.go]\nmodule = \"github.com/test/mylib\"");
    let (stream_test, plain_test) = render(&GoCodegen, &toml_src, None, |p| p.ends_with(".go"));

    let start = position(&stream_test, "stream, err := ");
    let read = position(&stream_test, "for range stream.Chan()");
    let stream_error = position(&stream_test, "err = stream.Err()");
    let judge = position(&stream_test, "expected an error, but call succeeded");
    assert!(
        start < read && read < stream_error && stream_error < judge,
        "the stream must be read, and its error taken, before the call is judged:\n{stream_test}"
    );
    assert!(
        !plain_test.contains("stream.Chan()"),
        "a call that is not a stream keeps its plain shape:\n{plain_test}"
    );
}

// ---- Java ----

#[test]
fn java_reads_the_stream_inside_assert_throws() {
    let toml_src = toml_for("java", "[crates.java]\npackage = \"dev.test.mylib\"");
    let (stream_test, plain_test) = render(&JavaCodegen, &toml_src, None, |p| p.ends_with(".java"));

    let open = position(&stream_test, "assertThrows(Exception.class, () -> {");
    let read = position(&stream_test, "_it.hasNext()");
    let close = position(&stream_test, "});");
    assert!(
        open < read && read < close,
        "the stream must be read inside the guarded lambda:\n{stream_test}"
    );
    assert!(
        !plain_test.contains(".iterator()"),
        "a call that is not a stream keeps its plain shape:\n{plain_test}"
    );
}

// ---- PHP ----

fn php_reads_the_stream_inside_the_guard(value: Option<&str>) {
    let toml_src = toml_for("php", "");
    let (stream_test, plain_test) = render(&PhpCodegen, &toml_src, value, |p| p.ends_with(".php"));

    let read = position(&stream_test, "foreach ($__stream as $__chunk)");
    let guard = if value.is_some() {
        position(&stream_test, "try {")
    } else {
        position(&stream_test, "$this->expectException(")
    };
    assert!(
        guard < read,
        "the stream must be read after the guard opens:\n{stream_test}"
    );
    assert!(
        !plain_test.contains("$__stream"),
        "a call that is not a stream keeps its plain shape:\n{plain_test}"
    );
}

#[test]
fn php_reads_the_stream_under_expect_exception() {
    php_reads_the_stream_inside_the_guard(None);
}

#[test]
fn php_reads_the_stream_inside_the_try_block() {
    php_reads_the_stream_inside_the_guard(Some("RateLimited"));
}

// ---- Elixir ----

#[test]
fn elixir_reads_the_stream_before_it_matches_the_error() {
    let toml_src = toml_for("elixir", "[crates.elixir]\napp_name = \"mylib\"");
    let (stream_test, plain_test) = render(&ElixirCodegen, &toml_src, None, |p| p.ends_with(".exs"));

    let read = position(&stream_test, "Enum.each(__stream");
    let rescue = position(&stream_test, "rescue");
    let judge = position(&stream_test, "assert {:error, _} = __stream_outcome");
    assert!(
        read < rescue && rescue < judge,
        "the stream must be read, and a raised error caught, before the match:\n{stream_test}"
    );
    assert!(
        !plain_test.contains("__stream_outcome"),
        "a call that is not a stream keeps its plain shape:\n{plain_test}"
    );
}

// ---- Swift ----

#[test]
fn swift_reads_the_stream_before_it_fails_the_test() {
    let toml_src = toml_for("swift", "[crates.e2e.packages.swift]\nname = \"MyLib\"");
    let (stream_test, plain_test) = render(&SwiftE2eCodegen, &toml_src, None, |p| p.ends_with(".swift"));

    let read = position(&stream_test, "for try await _ in __stream {}");
    let fail = position(&stream_test, "XCTFail(\"expected to throw\")");
    assert!(
        read < fail,
        "the stream must be read before the test fails:\n{stream_test}"
    );
    assert!(
        stream_test.contains("() async throws"),
        "a test that reads a stream must be async:\n{stream_test}"
    );
    assert!(
        !plain_test.contains("__stream"),
        "a call that is not a stream keeps its plain shape:\n{plain_test}"
    );
}

// ---- Kotlin ----

#[test]
fn kotlin_reads_the_stream_inside_assert_fails_with() {
    let toml_src = toml_for("kotlin", "[crates.kotlin]\npackage = \"dev.test.mylib\"");
    let (stream_test, plain_test) = render(&KotlinE2eCodegen, &toml_src, None, |p| p.ends_with(".kt"));

    let open = position(&stream_test, "assertFailsWith<Exception> {");
    let read = position(&stream_test, ".asSequence().toList()");
    let close = position(&stream_test, "        }");
    assert!(
        open < read && read < close,
        "the stream must be read inside the guarded block:\n{stream_test}"
    );
    assert!(
        !plain_test.contains(".asSequence()"),
        "a call that is not a stream keeps its plain shape:\n{plain_test}"
    );
}

// ---- Runtime: the generated test run against a stub library ----
//
// The tests above read the generated text. These run the generated Go and Elixir test files
// against a stub library whose stream ends either with no error or with an error. The test for
// a streaming call that must fail has to be red on the first stub and green on the second.

/// The whole generated test file of the stream fixtures, as emitted for one language.
fn generated_file<C: E2eCodegen>(codegen: &C, toml_src: &str, file_matches: fn(&str) -> bool) -> String {
    let cfg: NewAlefConfig = toml::from_str(toml_src).expect("config parses");
    let e2e = cfg.crates[0].e2e.clone().expect("e2e config present");
    let resolved = cfg.resolve().expect("config resolves").remove(0);
    let groups = vec![FixtureGroup {
        category: "streaming".to_string(),
        fixtures: vec![
            stream_fixture(STREAM_FIXTURE, "stream_call", None),
            stream_fixture(PLAIN_FIXTURE, "plain_call", None),
        ],
    }];
    let files = codegen
        .generate(&groups, &e2e, &resolved, &[], &[], &[], &[])
        .expect("generation succeeds");
    files
        .iter()
        .find(|f| file_matches(&f.path.to_string_lossy()) && f.content.contains("Midway") | f.content.contains("midway"))
        .map(|f| f.content.clone())
        .expect("a test file was emitted")
}

/// Find a toolchain binary. A missing binary skips the test unless CI marks it required.
fn toolchain(binary: &str, require_env: &str) -> Option<std::path::PathBuf> {
    match which::which(binary) {
        Ok(path) => Some(path),
        Err(_) if std::env::var_os(require_env).is_some() => {
            panic!("{require_env} is set but `{binary}` is absent")
        }
        Err(_) => None,
    }
}

/// Run `go test` over the generated Go file with a stub library. True when the tests pass.
fn go_test_passes(stream_ends_with_error: bool) -> Option<bool> {
    let go = toolchain("go", "ALEF_REQUIRE_GO")?;
    let toml_src = toml_for("go", "[crates.go]\nmodule = \"github.com/test/mylib\"");
    let test_source = generated_file(&GoCodegen, &toml_src, |p| p.ends_with("streaming_test.go"));

    let root = tempfile::tempdir().expect("temp dir");
    let lib = root.path().join("lib");
    let e2e = root.path().join("e2e");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::create_dir_all(&e2e).unwrap();
    std::fs::write(lib.join("go.mod"), "module github.com/test/mylib\n\ngo 1.21\n").unwrap();
    let stream_error = if stream_ends_with_error {
        "errors.New(\"rate limit reached\")"
    } else {
        "nil"
    };
    std::fs::write(
        lib.join("lib.go"),
        format!(
            r#"package mylib

import (
	"context"
	"errors"
)

type Stream struct {{
	ch  chan string
	err error
}}

func (s *Stream) Chan() <-chan string {{ return s.ch }}

// Err answers only after the channel closes, as a stream that fails while it is read.
func (s *Stream) Err() error {{ return s.err }}

func StreamCallWithContext(ctx context.Context) (*Stream, error) {{
	ch := make(chan string, 2)
	ch <- "a"
	ch <- "b"
	close(ch)
	return &Stream{{ch: ch, err: {stream_error}}}, nil
}}

func PlainCall() (string, error) {{ return "", errors.New("plain failure") }}
"#
        ),
    )
    .unwrap();
    std::fs::write(
        e2e.join("go.mod"),
        "module github.com/test/mylib/e2e\n\ngo 1.21\n\nrequire github.com/test/mylib v0.0.0\n\nreplace github.com/test/mylib => ../lib\n",
    )
    .unwrap();
    std::fs::write(e2e.join("streaming_test.go"), test_source).unwrap();

    let output = std::process::Command::new(go)
        .args(["test", "./..."])
        .current_dir(&e2e)
        .env("GOWORK", "off")
        .env("GOFLAGS", "-mod=mod")
        .env("GOPROXY", "off")
        .output()
        .expect("go runs");
    Some(output.status.success())
}

#[test]
fn go_test_is_red_when_the_stream_ends_with_no_error() {
    if let Some(passes) = go_test_passes(false) {
        assert!(!passes, "a stream that ends clean must fail the generated Go test");
    }
}

#[test]
fn go_test_is_green_when_the_stream_ends_in_an_error() {
    if let Some(passes) = go_test_passes(true) {
        assert!(passes, "a stream that ends in an error must pass the generated Go test");
    }
}

/// Run the generated Elixir test file with a stub `MyLib`. True when the tests pass.
fn elixir_test_passes(stream_ends_with_error: bool) -> Option<bool> {
    let elixir = toolchain("elixir", "ALEF_REQUIRE_ELIXIR")?;
    let toml_src = toml_for("elixir", "[crates.elixir]\napp_name = \"mylib\"");
    let test_source = generated_file(&ElixirCodegen, &toml_src, |p| p.ends_with("streaming_test.exs"));

    let root = tempfile::tempdir().expect("temp dir");
    let test_path = root.path().join("streaming_test.exs");
    std::fs::write(&test_path, test_source).unwrap();
    let stream = if stream_ends_with_error {
        "Stream.map([1, 2, 3], fn 3 -> raise \"rate limit reached\"; n -> n end)"
    } else {
        "[1, 2]"
    };
    let script = root.path().join("run.exs");
    std::fs::write(
        &script,
        format!(
            r#"defmodule MyLib do
  def stream_call, do: {{:ok, {stream}}}
  def plain_call, do: {{:error, :plain_failure}}
end

ExUnit.start(autorun: false)
Code.require_file({test_path:?})
%{{failures: failures}} = ExUnit.run()
System.halt(if failures == 0, do: 0, else: 1)
"#
        ),
    )
    .unwrap();

    let output = std::process::Command::new(elixir)
        .arg(&script)
        .output()
        .expect("elixir runs");
    Some(output.status.success())
}

#[test]
fn elixir_test_is_red_when_the_stream_ends_with_no_error() {
    if let Some(passes) = elixir_test_passes(false) {
        assert!(!passes, "a stream that ends clean must fail the generated Elixir test");
    }
}

#[test]
fn elixir_test_is_green_when_the_stream_ends_in_an_error() {
    if let Some(passes) = elixir_test_passes(true) {
        assert!(passes, "a stream that ends in an error must pass the generated Elixir test");
    }
}

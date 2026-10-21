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

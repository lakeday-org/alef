use alef::core::config::NewAlefConfig;
use alef::e2e::codegen::{E2eCodegen, dart::DartE2eCodegen, ruby::RubyCodegen};
use alef::e2e::fixture::{Assertion, Fixture, FixtureGroup};
use std::io::ErrorKind;

fn configuration() -> (alef::e2e::config::E2eConfig, alef::core::config::ResolvedCrateConfig) {
    let config: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["ruby", "dart"]
[[crates]]
name = "sample_lib"
sources = ["src/lib.rs"]
[crates.e2e]
fixtures = "fixtures"
output = "e2e"
[crates.e2e.call]
function = "stream_text"
module = "SampleLib"
result_var = "result"
args = []
[crates.e2e.call.streaming]
enabled = true
item_type = "StreamChunk"
[crates.e2e.call.overrides.dart]
module = "SampleLib"
function = "streamText"
"#,
    )
    .expect("config parses");
    let e2e = config.crates[0].e2e.clone().expect("e2e config");
    let resolved = config.resolve().expect("config resolves").remove(0);
    (e2e, resolved)
}

fn fixture_groups() -> [FixtureGroup; 1] {
    [FixtureGroup {
        category: "streaming".into(),
        fixtures: vec![Fixture {
            id: "strict_terminal_boundary".into(),
            assertions: vec![
                Assertion {
                    assertion_type: "not_empty".into(),
                    field: Some("stream_content".into()),
                    ..Assertion::default()
                },
                Assertion {
                    assertion_type: "is_true".into(),
                    field: Some("no_chunks_after_done".into()),
                    ..Assertion::default()
                },
            ],
            ..Fixture::default()
        }],
    }]
}

fn render(language: &str) -> String {
    let (e2e, resolved) = configuration();
    let groups = fixture_groups();
    let generator: &dyn E2eCodegen = match language {
        "ruby" => &RubyCodegen,
        "dart" => &DartE2eCodegen,
        _ => panic!("unexpected language"),
    };
    let files = generator
        .generate(&groups, &e2e, &resolved, &[], &[], &[], &[])
        .expect("generation succeeds");
    let marker = if language == "ruby" {
        "    done_at ="
    } else {
        "    final _doneAt ="
    };
    let blocks: Vec<_> = files
        .iter()
        .filter_map(|file| {
            file.content.find(marker).map(|start| {
                file.content[start..]
                    .lines()
                    .take(if language == "ruby" { 3 } else { 2 })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        })
        .collect();
    assert_eq!(blocks.len(), 1, "exactly one generated terminal-boundary assertion");
    blocks.into_iter().next().expect("one block")
}

fn run(language: &str, extension: &str, template: &str, marker: &str) {
    let script = template.replace(marker, &render(language));
    assert!(
        !script.contains("GENERATED_ASSERTION"),
        "fixture must contain the actual generated assertion"
    );
    let dir = tempfile::tempdir().expect("owned runtime directory");
    let path = dir.path().join(format!("boundary.{extension}"));
    std::fs::write(&path, script).expect("write runtime fixture");
    let output = match alef::core::tool_command(language)
        .arg(&path)
        .current_dir(dir.path())
        .output()
    {
        Ok(output) => output,
        Err(error)
            if error.kind() == ErrorKind::NotFound
                && std::env::var_os("ALEF_REQUIRE_STREAM_BOUNDARY_TOOLS").is_none() =>
        {
            return;
        }
        Err(error) => panic!("cannot execute {language} terminal-boundary fixture: {error}"),
    };
    assert!(
        output.status.success(),
        "{language}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "BOUNDARY_CASES=10;OLD_PREDICATE_FALSE_PASS=1"
    );
}

#[test]
fn ruby_generated_boundary_rejects_every_chunk_after_the_first_terminal() {
    run(
        "ruby",
        "rb",
        include_str!("fixtures/stream_terminal_boundary.rb"),
        "# GENERATED_ASSERTION ~keep",
    );
}

#[test]
fn dart_generated_boundary_rejects_every_chunk_after_the_first_terminal() {
    run(
        "dart",
        "dart",
        include_str!("fixtures/stream_terminal_boundary.dart"),
        "// GENERATED_ASSERTION ~keep",
    );
}

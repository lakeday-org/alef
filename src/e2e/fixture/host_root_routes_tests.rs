use super::Fixture;
use crate::core::config::ResolvedCrateConfig;
use crate::e2e::codegen::{E2eCodegen, swift::SwiftE2eCodegen};
use crate::e2e::config::E2eConfig;
use crate::e2e::fixture::FixtureGroup;

fn file_route_fixture() -> Fixture {
    serde_json::from_value(serde_json::json!({
        "id": "file_redirect", "description": "File-backed redirect",
        "input": {"mock_responses": [
            {"path": "/", "status_code": 200, "body_file": "html/refresh.html"},
            {"path": "/target", "status_code": 200, "body_inline": "finished"}
        ]}, "assertions": []
    }))
    .expect("valid file-backed fixture")
}

#[test]
fn file_backed_routes_consult_runtime_origin_selection() {
    let fixture = file_route_fixture();
    assert!(
        fixture.has_host_root_route(),
        "opaque file content may resolve against the origin root"
    );
    let mut inline = fixture.clone();
    inline.input["mock_responses"][0] = serde_json::json!({
        "path": "/", "status_code": 200, "body_inline": "plain text"
    });
    assert!(
        !inline.has_host_root_route(),
        "plain inline bodies retain namespaced routing"
    );
    for invalid in [serde_json::Value::Null, serde_json::json!(""), serde_json::json!(17)] {
        let mut fixture = fixture.clone();
        fixture.input["mock_responses"][0]["body_file"] = invalid;
        assert!(!fixture.has_host_root_route());
    }
}

fn swift_url_statement() -> String {
    let config: E2eConfig = serde_json::from_value(serde_json::json!({
        "call": {"function": "fetch_page", "args": [
            {"name": "url", "field": "url", "type": "mock_url"}
        ]}
    }))
    .expect("valid mock URL configuration");
    let files = SwiftE2eCodegen
        .generate(
            &[FixtureGroup {
                category: "routing".to_string(),
                fixtures: vec![file_route_fixture()],
            }],
            &config,
            &ResolvedCrateConfig {
                name: "sample".to_string(),
                ..Default::default()
            },
            &[],
            &[],
            &[],
            &[],
        )
        .expect("generate Swift fixture");
    let statements: Vec<_> = files
        .iter()
        .flat_map(|file| file.content.lines())
        .filter(|line| line.trim_start().starts_with("let url = "))
        .collect();
    assert_eq!(
        statements.len(),
        1,
        "exactly one generated URL argument must be checked"
    );
    statements[0].trim().to_string()
}

#[test]
fn swift_file_backed_route_preserves_dedicated_environment_fallback() {
    let statement = swift_url_statement();
    assert!(
        statement.contains("environment[\"MOCK_SERVER_FILE_REDIRECT\"]"),
        "{statement}"
    );
    assert!(statement.contains("?? (AlefE2EMockServer.baseURL"), "{statement}");
}

fn compile_swift_url_probe(directory: &std::path::Path) -> std::path::PathBuf {
    let statement = swift_url_statement();
    let source = [
        "import Foundation\nenum AlefE2EMockServer { static let baseURL = \"http://127.0.0.1:4100\" }",
        &statement,
        "exit(url == CommandLine.arguments[1] ? 0 : 1)",
    ]
    .join("\n");
    let path = directory.join("routing.swift");
    std::fs::write(&path, source).expect("write emitted Swift URL probe");
    let binary = directory.join(format!("routing{}", std::env::consts::EXE_SUFFIX));
    let output = std::process::Command::new("swiftc")
        .arg(&path)
        .arg("-o")
        .arg(&binary)
        .current_dir(directory)
        .output()
        .expect("compile actual emitted Swift URL");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    binary
}

fn run_swift_url_probe(binary: &std::path::Path, origin: Option<&str>, expected: &str) -> bool {
    let mut command = std::process::Command::new(binary);
    command
        .arg(expected)
        .current_dir(binary.parent().expect("owned probe directory"))
        .env_remove("MOCK_SERVER_FILE_REDIRECT");
    if let Some(origin) = origin {
        command.env("MOCK_SERVER_FILE_REDIRECT", origin);
    }
    command.output().expect("execute emitted Swift URL").status.success()
}

#[test]
#[ignore = "requires the Swift compiler"]
fn native_swift_file_backed_url_selects_dedicated_and_shared_origins() {
    let directory = tempfile::tempdir().expect("Swift routing probe directory");
    let binary = compile_swift_url_probe(directory.path());
    for (origin, expected, success) in [
        (Some("http://127.0.0.1:4200"), "http://127.0.0.1:4200", true),
        (None, "http://127.0.0.1:4100/fixtures/file_redirect", true),
        (
            Some("http://127.0.0.1:4200"),
            "http://127.0.0.1:4100/fixtures/file_redirect",
            false,
        ),
    ] {
        assert_eq!(
            run_swift_url_probe(&binary, origin, expected),
            success,
            "{origin:?}: {expected}"
        );
    }
}

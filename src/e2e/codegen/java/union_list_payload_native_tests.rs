use crate::core::backend::Backend;
use crate::core::config::ResolvedCrateConfig;
use crate::e2e::codegen::go::test_backend::enum_collection_support as support;

#[test]
#[ignore = "native Java proof compiles actual generated DTOs and full E2E assertions"]
fn issue688_java_collection_assertions_execute_producer_backed_cases() {
    for wire in [false, true] {
        verify_native(wire);
    }
}

fn assertions(api: &crate::core::ir::ApiSurface) -> String {
    let files = support::generate_collection_call(&super::JavaCodegen, api);
    assertion_statements(&files, 11)
}

fn assertion_statements(files: &[crate::core::backend::GeneratedFile], expected: usize) -> String {
    let output = &files
        .iter()
        .find(|file| file.path.file_name().is_some_and(|name| name == "PayloadTest.java"))
        .expect("category test emitted")
        .content;
    assert!(!output.contains(".parts()"), "{output}");
    let mut statements = Vec::new();
    let mut pending = String::new();
    for line in output.lines() {
        if pending.is_empty() && !line.trim_start().starts_with("assert") && !line.trim_start().starts_with("fail(") {
            continue;
        }
        pending.push_str(line);
        pending.push('\n');
        if line.trim_end().ends_with(';') {
            statements.push(std::mem::take(&mut pending));
        }
    }
    assert!(pending.is_empty(), "unterminated generated assertion: {pending}");
    assert_eq!(statements.len(), expected, "{output}");
    statements.join("\n")
}

fn standard_checks(api: &crate::core::ir::ApiSurface) -> Vec<minijinja::Value> {
    [
        ("minimum", "count_min", Some(serde_json::json!(2)), 3),
        ("empty", "is_empty", None, 3),
        ("nonempty", "not_empty", None, 3),
        ("unsupported", "future_kind", None, 1),
    ]
    .into_iter()
    .map(|(name, kind, value, rows)| {
        let assertion = crate::e2e::fixture::Assertion {
            assertion_type: kind.to_string(),
            field: Some("choices[0].message.content.parts".to_string()),
            value,
            ..Default::default()
        };
        let files = support::generate_call_with_assertions(&super::JavaCodegen, api, vec![assertion]);
        crate::alef_context! { name => name, assertions => assertion_statements(&files, rows) }
    })
    .collect()
}

fn classpath() -> String {
    let root = std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join(".m2/repository");
    let artifacts = [
        "jackson-databind",
        "jackson-core",
        "jackson-annotations",
        "junit-jupiter-api",
        "opentest4j",
        "apiguardian-api",
        "jspecify",
        "junit-platform-commons",
    ];
    let mut jars = Vec::new();
    for artifact in artifacts {
        let mut matches = walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(&format!("{artifact}-")) && name.ends_with(".jar"))
            })
            .map(|entry| entry.into_path())
            .collect::<Vec<_>>();
        matches.sort();
        jars.push(matches.pop().unwrap_or_else(|| panic!("missing cached {artifact} jar")));
    }
    std::env::join_paths(jars).unwrap().to_string_lossy().into_owned()
}

fn write_bindings(api: &crate::core::ir::ApiSurface, config: &ResolvedCrateConfig, directory: &std::path::Path) {
    let files = crate::backends::java::JavaBackend
        .generate_bindings(api, config)
        .unwrap();
    let exception = format!(
        "{}Exception.java",
        crate::backends::java::naming::main_class_name(&api.crate_name)
    );
    let mut count = 0;
    for file in files {
        let Some(name) = file.path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name != exception
            && !matches!(
                name,
                "Body.java" | "Piece.java" | "Reply.java" | "Choice.java" | "Message.java" | "JsonMapperFactory.java"
            )
        {
            continue;
        }
        let source = file
            .content
            .lines()
            .filter(|line| !line.starts_with("package "))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(directory.join(name), source).unwrap();
        count += 1;
    }
    assert_eq!(count, 7, "actual generated DTO dependencies");
}

fn verify_native(wire: bool) {
    let directory = tempfile::tempdir().unwrap();
    let api = support::call_surface(wire);
    let config = ResolvedCrateConfig {
        name: "sample".to_string(),
        ..Default::default()
    };
    let (java, runtime_path) = compile_probe(&api, &config, directory.path());
    support::verify_cases(
        support::core_cases(wire),
        [
            "PASS:3",
            "expected: <2>",
            "expected: <hello>",
            "expected: < world>",
            "expected list variant",
            "expected content",
        ],
        |case| {
            std::process::Command::new(&java)
                .args(["-cp", &runtime_path, "Probe", &case.to_string()])
                .current_dir(directory.path())
                .output()
                .unwrap()
        },
    );
    verify_collection_checks(wire, directory.path(), &java, &runtime_path);
}

fn verify_collection_checks(wire: bool, directory: &std::path::Path, java: &std::path::Path, classpath: &str) {
    let cases = support::core_collection_cases(wire);
    assert_eq!(cases.len(), 4);
    let checks = [
        ("minimum", [false, true, false, false], "expected at least 2 elements"),
        ("empty", [true, false, false, false], "expected empty collection"),
        ("nonempty", [false, true, false, false], "expected non-empty collection"),
        ("unsupported", [false; 4], "unsupported enum collection assertion"),
    ];
    let mut checked = 0;
    for (name, successes, mismatch) in checks {
        for (index, case) in cases.iter().enumerate() {
            let output = std::process::Command::new(java)
                .args(["-cp", classpath, "Probe", &case.to_string(), name])
                .current_dir(directory)
                .output()
                .unwrap();
            let evidence = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                output.status.success(),
                successes[index],
                "{name} case {index}: {evidence}"
            );
            let marker = expected_marker(successes[index], name, index, mismatch);
            assert!(evidence.contains(marker), "{name} case {index}: {evidence}");
            checked += 1;
        }
    }
    assert_eq!(checked, 16);
}

fn compile_probe(
    api: &crate::core::ir::ApiSurface,
    config: &ResolvedCrateConfig,
    directory: &std::path::Path,
) -> (std::path::PathBuf, String) {
    write_bindings(api, config, directory);
    let source = crate::e2e::template_env::render(
        "java/list_variant_native_probe.jinja",
        crate::alef_context! { assertions => assertions(api), checks => standard_checks(api) },
    );
    std::fs::write(directory.join("Probe.java"), source).unwrap();
    let classpath = classpath();
    let javac = support::tool("javac", directory);
    let sources = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    let output = std::process::Command::new(javac)
        .arg("-cp")
        .arg(&classpath)
        .args(sources)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let java = support::tool("java", directory);
    let runtime_path = format!("{}:{}", directory.display(), classpath);
    (java, runtime_path)
}

fn expected_marker<'a>(success: bool, name: &str, index: usize, mismatch: &'a str) -> &'a str {
    if success {
        "PASS:collection"
    } else if name == "unsupported" || index < 2 {
        mismatch
    } else if index == 2 {
        "expected list variant"
    } else {
        "expected content"
    }
}

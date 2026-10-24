use std::path::Path;

fn unused_imports(root: &Path, source: &str) -> Vec<String> {
    std::fs::write(root.join("GeneratedTest.kt"), source).unwrap();
    let mut command = std::process::Command::new("task");
    command.arg("lint").arg("--silent").current_dir(root);
    let output = crate::snippets::validators::run_command_streams(&mut command, 60).unwrap();
    let report: serde_json::Value = serde_json::from_str(&output.stdout)
        .unwrap_or_else(|error| panic!("ktlint JSON: {error}: {}\n{}", output.stdout, output.stderr));
    let files = report.as_array().unwrap();
    assert!(files.len() <= 1, "exactly one generated file is passed to ktlint");
    if files.is_empty() {
        assert!(output.success, "{}", output.stderr);
        return Vec::new();
    }
    assert!(!output.success, "ktlint violations must fail the gate");
    assert!(files[0]["file"].as_str().unwrap().ends_with("GeneratedTest.kt"));
    let errors = files[0]["errors"].as_array().unwrap();
    assert!(!errors.is_empty(), "reported artifact must have actual diagnostics");
    for error in errors {
        assert_eq!(error["rule"], "standard:no-unused-imports", "{error}");
        assert_eq!(error["message"], "Unused import", "{error}");
    }
    let lines: Vec<_> = source.lines().collect();
    errors
        .iter()
        .map(|error| lines[error["line"].as_u64().unwrap() as usize - 1].to_string())
        .collect()
}

#[test]
#[ignore = "requires ktlint and a JDK"]
fn native_ktlint_checks_discriminated_imports_in_full_generated_tests() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    for (name, contents) in [
        (
            "Taskfile.yml",
            include_str!("../../../../../tests/fixtures/kotlin_assertion_imports/Taskfile.yml"),
        ),
        (
            ".editorconfig",
            include_str!("../../../../../tests/fixtures/kotlin_assertion_imports/.editorconfig"),
        ),
    ] {
        std::fs::write(root.join(name), contents).unwrap();
    }
    let mut checked = 0;
    for (skipped, runnable) in [(true, false), (true, true), (false, true)] {
        let source = super::tests::generate(skipped, runnable);
        let unused = unused_imports(root, &source);
        assert!(unused.is_empty(), "all generated imports must be used: {unused:?}");
        let planted = source.replacen(
            "package dev.sample.e2e\n",
            "package dev.sample.e2e\nimport kotlin.collections.ArrayList\n",
            1,
        );
        assert_ne!(source, planted, "the valid unused import must be planted");
        assert!(unused_imports(root, &planted).contains(&"import kotlin.collections.ArrayList".to_string()));
        checked += 1;
    }
    assert_eq!(checked, 3, "three positive and three planted-import controls must run");
}

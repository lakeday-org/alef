use alef::backends::java::JavaBackend;
use alef::core::backend::Backend;
use alef::core::config::NewAlefConfig;
use std::path::Path;

fn render(template: &str, context: minijinja::Value) -> String {
    let mut environment = minijinja::Environment::new();
    alef::template::configure_env(&mut environment);
    environment.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    environment.add_template("fixture", template).unwrap();
    environment.get_template("fixture").unwrap().render(context).unwrap()
}

fn record(root: &Path, first: usize, middle: usize, last: usize, legacy: bool, history: bool) {
    let source = render(
        include_str!("fixtures/java_constructor_history/core.rs.jinja"),
        minijinja::context! { first, middle, last, legacy, history },
    );
    let source_path = root.join("lib.rs");
    std::fs::write(&source_path, source).unwrap();
    let api = alef::extract::extractor::extract(&[&source_path], "fixture", "1.0.0", None).unwrap();
    assert_eq!(api.types.len(), 1);
    let config: NewAlefConfig = toml::from_str(
        "[workspace]\nlanguages = [\"java\"]\n[[crates]]\nname = \"fixture\"\nsources = [\"lib.rs\"]\n[crates.java]\npackage = \"fixture\"\n[crates.java.dto]\nbuilder = \"never\"\n",
    ).unwrap();
    let files = JavaBackend
        .generate_bindings(&api, &config.resolve().unwrap()[0])
        .unwrap();
    let records = files
        .iter()
        .filter(|file| file.path.ends_with("Options.java"))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    let package = root.join("fixture");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("Options.java"), &records[0].content).unwrap();
    let annotations = root.join("org/jspecify/annotations");
    std::fs::create_dir_all(&annotations).unwrap();
    std::fs::write(
        annotations.join("Nullable.java"),
        include_str!("fixtures/java_constructor_history/Nullable.java"),
    )
    .unwrap();
}

fn compile(root: &Path) -> std::process::Output {
    alef::core::tool_command("javac")
        .args([
            "-d",
            ".",
            "fixture/Options.java",
            "fixture/Client.java",
            "org/jspecify/annotations/Nullable.java",
        ])
        .current_dir(root)
        .output()
        .expect("javac must execute: this regression cannot silently skip its compiler gate")
}

fn assert_native_client(first: usize, middle: usize, last: usize, legacy: bool) -> usize {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    record(root, first, middle, last, legacy, false);
    let client = render(
        include_str!("fixtures/java_constructor_history/Client.java.jinja"),
        minijinja::context! { arities => [first, middle, last], last },
    );
    std::fs::write(root.join("fixture/Client.java"), client).unwrap();
    let negative = compile(root);
    let diagnostics = String::from_utf8_lossy(&negative.stderr);
    assert!(
        !negative.status.success(),
        "missing history must reject the old middle constructor"
    );
    assert!(
        diagnostics.contains("no suitable constructor") || diagnostics.contains("invalid constructor reference"),
        "{diagnostics}"
    );
    record(root, first, middle, last, legacy, true);
    let positive = compile(root);
    assert!(
        positive.status.success(),
        "{}",
        String::from_utf8_lossy(&positive.stderr)
    );
    let run = alef::core::tool_command("java")
        .args(["-cp", ".", "fixture.Client"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).trim(),
        format!("checked={}", last * 6)
    );
    last * 6
}

#[test]
fn recorded_java_constructor_calls_and_method_references_survive_multiple_extensions() {
    let mut actual_checks = 0;
    let mut clients = 0;
    for (first, middle, last) in [(1, 2, 3), (13, 20, 24), (55, 56, 57)] {
        for legacy in [true, false] {
            actual_checks += assert_native_client(first, middle, last, legacy);
            clients += 1;
        }
    }
    assert_eq!(clients, 6);
    assert_eq!(
        actual_checks, 1008,
        "six native clients must check every supplied/defaulted field"
    );
}

use std::path::Path;

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn fixture(generated: &str, enabled: bool) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write(
        root,
        "sample-core/Cargo.toml",
        "[package]\nname='sample-core'\nversion='0.1.0'\nedition='2024'\n",
    );
    write(root, "sample-core/src/lib.rs", include_str!("core.rs"));
    write(
        root,
        "sample-py/Cargo.toml",
        "[package]\nname='capsule-gil-probe'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nsample-core={path='../sample-core'}\npyo3={version='0.29',features=['auto-initialize']}\nserde={version='1',features=['derive']}\nserde_json='1'\n",
    );
    let source = format!(
        "{generated}\nconst EXPECT_RELEASE: bool = {enabled};\n{}",
        include_str!("probe.rs")
    );
    write(root, "sample-py/src/lib.rs", &source);
    write(root, "sample-py/src/probe.py", include_str!("probe.py"));
    write(
        root,
        "sample-py/mise.toml",
        "[tasks.test]\nrun = \"cargo test --quiet --lib\"\n",
    );
    directory
}

pub fn run(generated: &str, enabled: bool) {
    let directory = fixture(generated, enabled);
    let root = directory.path();
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/pyo3-capsule-gil-probe");
    let output = super::mise_task::mise_run(&root.join("sample-py"), "test")
        .env("CARGO_TARGET_DIR", target)
        .env("RUSTC_WRAPPER", "")
        .output()
        .unwrap();
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "release_gil={enabled}: {diagnostics}");
    assert!(
        diagnostics.contains("1 passed"),
        "runtime probe did not run: {diagnostics}"
    );
}

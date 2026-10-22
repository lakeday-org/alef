#[test]
fn borrowed_registry_reuses_native_static_text_and_rejects_unregistered_text() {
    let registry = super::render(
        "rust_error_static_text_registry.rs.jinja",
        crate::alef_context! { module_name => "original_text" },
    );
    assert!(!registry.contains("Box::leak"), "{registry}");
    let registry = registry.replace("#[frb(ignore)]\n", "");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.rs");
    let binary = directory.path().join(format!("probe{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source, format!("{registry}\n{}", PROBE)).unwrap();
    let compile = std::process::Command::new(which::which("rustc").expect("Rust compiler is required"))
        .arg("--edition=2024")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(compile.status.success(), "{}", String::from_utf8_lossy(&compile.stderr));
    let run = std::process::Command::new(binary).output().unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}

const PROBE: &str = r#"
fn main() {
    static ORIGINAL: &str = "native error text";
    for _ in 0..100 {
        let mirror = original_text::register(ORIGINAL);
        let restored = original_text::lookup(&mirror).unwrap();
        assert!(std::ptr::eq(restored, ORIGINAL));
    }
    let unknown = original_text::lookup("new Dart text").unwrap_err();
    assert!(unknown.contains("reconstructing borrowed error"));
    assert_eq!(original_text::lookup(ORIGINAL).unwrap(), ORIGINAL);
}
"#;

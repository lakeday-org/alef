use alef::Language;
use alef::backends::pyo3::Pyo3Backend;
use alef::core::backend::Backend;
use alef::core::config::NewAlefConfig;
use std::path::Path;

const CORE: &str = r#"
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum Selection {
    Named { value: String },
}
pub fn selected_value(selection: Selection) -> String {
    match selection {
        Selection::Named { value } => value,
    }
}
pub async fn selected_value_async(selection: Selection) -> String {
    selected_value(selection)
}
"#;

#[test]
fn default_python_dependencies_compile_owned_data_enums_and_async_functions() {
    let directory = tempfile::tempdir().expect("consumer directory");
    write_core(directory.path());
    let source = directory.path().join("src/lib.rs");
    let api = alef::extract::extractor::extract(&[&source], "sample-core", "0.1.0", None).unwrap();
    assert_eq!(api.enums.len(), 1);
    assert_eq!(api.functions.len(), 2);
    let parsed: NewAlefConfig = toml::from_str(
        "[workspace]\nlanguages=['python']\n[[crates]]\nname='sample-core'\nsources=['src/lib.rs']\n[crates.python]\nmodule_name='_sample_core'\n",
    )
    .unwrap();
    let mut config = parsed.resolve().unwrap().remove(0);
    config.workspace_root = Some(directory.path().to_path_buf());
    let files = alef::scaffold::scaffold(&api, &config, &[Language::Python]).unwrap();
    let manifests: Vec<_> = files.iter().filter(|file| file.path.ends_with("Cargo.toml")).collect();
    assert_eq!(manifests.len(), 1, "exactly one generated binding manifest");
    let manifest = &manifests[0];
    let parsed_manifest: toml::Value = toml::from_str(&manifest.content).unwrap();
    for name in ["pyo3", "pyo3-async-runtimes"] {
        assert_eq!(parsed_manifest["dependencies"][name]["version"].as_str(), Some("0.30"));
    }
    let manifest_path = directory.path().join(&manifest.path);
    std::fs::create_dir_all(manifest_path.parent().unwrap().join("src")).unwrap();
    std::fs::write(&manifest_path, &manifest.content).unwrap();
    let bindings = Pyo3Backend.generate_bindings(&api, &config).unwrap();
    let libraries: Vec<_> = bindings.iter().filter(|file| file.path.ends_with("lib.rs")).collect();
    assert_eq!(libraries.len(), 1);
    std::fs::write(
        manifest_path.parent().unwrap().join("src/lib.rs"),
        &libraries[0].content,
    )
    .unwrap();
    compile_consumer(&manifest_path, directory.path());
}

fn write_core(root: &Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), CORE).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='sample-core'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nserde={version='1',features=['derive']}\n",
    )
    .unwrap();
}

fn compile_consumer(manifest: &Path, root: &Path) {
    let output = std::process::Command::new("cargo")
        .args(["check", "--quiet", "--manifest-path"])
        .arg(manifest)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env("CARGO_BUILD_JOBS", "2")
        .env("RUSTC_WRAPPER", "")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let lock: toml::Value =
        toml::from_str(&std::fs::read_to_string(manifest.parent().unwrap().join("Cargo.lock")).unwrap()).unwrap();
    for name in ["pyo3", "pyo3-async-runtimes"] {
        let packages: Vec<_> = lock["package"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|package| package["name"].as_str() == Some(name))
            .collect();
        assert_eq!(packages.len(), 1, "one resolved package for {name}");
        assert!(packages[0]["version"].as_str().unwrap().starts_with("0.30."));
    }
}

use alef::adapters::streaming::generate_body;
use alef::core::config::{AdapterConfig, AdapterPattern, Language, NewAlefConfig};

#[test]
fn php_stream_types_preserve_external_paths_and_unit_defaults() {
    let config = config();
    let mut adapter = adapter();
    for (item, error, expected) in [
        (None, None, "Result<(), anyhow::Error>"),
        (
            Some("Event"),
            Some("ClientError"),
            "Result<sample_core::Event, sample_core::ClientError>",
        ),
        (
            Some("other::Event"),
            Some("other::Error"),
            "Result<other::Event, other::Error>",
        ),
    ] {
        adapter.item_type = item.map(str::to_string);
        adapter.error_type = error.map(str::to_string);
        let (_, handle) = generate_body(&adapter, Language::Php, &config).unwrap();
        let handle = handle.unwrap();
        assert!(handle.contains(expected), "{handle}");
        syn::parse_file(&handle).expect("generated stream handle is valid Rust");
    }
}

fn config() -> alef::core::config::ResolvedCrateConfig {
    let config: NewAlefConfig = toml::from_str(
        "[workspace]\nlanguages = [\"php\"]\n[[crates]]\nname = \"sample-core\"\nsources = [\"src/lib.rs\"]",
    )
    .unwrap();
    config.resolve().unwrap().remove(0)
}

fn adapter() -> AdapterConfig {
    AdapterConfig {
        name: "events".into(),
        pattern: AdapterPattern::Streaming,
        core_path: "events".into(),
        params: vec![],
        returns: None,
        error_type: None,
        owner_type: Some("Client".into()),
        item_type: None,
        gil_release: false,
        trait_name: None,
        trait_method: None,
        detect_async: false,
        request_type: None,
        skip_languages: vec![],
    }
}

#[test]
fn php_default_stream_manifest_resolves_the_emitted_type() {
    let mut config = config();
    let adapter = adapter();
    let (_, handle) = generate_body(&adapter, Language::Php, &config).unwrap();
    let handle = handle.unwrap();
    let field_type = handle
        .lines()
        .find_map(|line| line.trim().strip_prefix("stream: "))
        .unwrap();
    let source = format!(
        "fn main() {{ let _: {} = Default::default(); }}",
        field_type.trim_end_matches(',')
    );
    config.adapters.push(adapter);
    let api = alef::core::ir::ApiSurface {
        version: "0.1.0".into(),
        ..Default::default()
    };
    let files = alef::scaffold::scaffold(&api, &config, &[Language::Php]).unwrap();
    let cargo = files.iter().find(|file| file.path.ends_with("Cargo.toml")).unwrap();
    let manifest: toml::Value = toml::from_str(&cargo.content).unwrap();
    let dependencies = manifest["dependencies"].as_table().unwrap();
    assert!(dependencies.contains_key("anyhow"), "{}", cargo.content);
    let dependencies = ["anyhow", "futures-util"]
        .into_iter()
        .map(|name| (name.to_string(), dependencies[name].clone()))
        .collect();
    check_stream_type_resolution(&source, dependencies);
}

fn check_stream_type_resolution(source: &str, dependencies: toml::map::Map<String, toml::Value>) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("src")).unwrap();
    std::fs::write(directory.path().join("src/main.rs"), source).unwrap();
    let mut executed = 0;
    for with_anyhow in [false, true] {
        let mut dependencies = dependencies.clone();
        if !with_anyhow {
            dependencies.remove("anyhow");
        }
        let mut manifest = toml::toml! {
            [package]
            name = "stream_type_probe"
            version = "0.1.0"
            edition = "2024"
        };
        manifest.insert("dependencies".into(), toml::Value::Table(dependencies));
        std::fs::write(directory.path().join("Cargo.toml"), toml::to_string(&manifest).unwrap()).unwrap();
        let output = std::process::Command::new("cargo")
            .args(["check", "--offline", "--quiet"])
            .current_dir(directory.path())
            .env("CARGO_TARGET_DIR", directory.path().join("target"))
            .output()
            .unwrap();
        executed += 1;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.success(), with_anyhow, "{stderr}");
        if !with_anyhow {
            assert!(stderr.contains("anyhow") && stderr.contains("E0433"), "{stderr}");
        }
    }
    assert_eq!(executed, 2);
}

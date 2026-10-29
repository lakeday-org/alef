use alef::publish::package::{BuildProfile, find_built_artifact};
use alef::publish::platform::RustTarget;
use std::path::{Path, PathBuf};

#[path = "support/mise_task.rs"]
mod mise_task;

fn isolated_from_ambient_target_directory(test_name: &str) -> bool {
    if std::env::var("ALEF_STAGING_CONFIG_CHILD").as_deref() == Ok(test_name) {
        return true;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test_name, "--nocapture"])
        .env("ALEF_STAGING_CONFIG_CHILD", test_name)
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    false
}

fn workspace(root: &Path, target_dir: &Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join(".cargo")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = 'staging-probe'\nversion = '0.1.0'\nedition = '2024'\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"), "").unwrap();
    std::fs::write(
        root.join(".cargo/config.toml"),
        format!("[build]\ntarget-dir = '{}'\n", target_dir.display()),
    )
    .unwrap();
}

fn artifact(directory: &Path, contents: &[u8]) -> PathBuf {
    std::fs::create_dir_all(directory).unwrap();
    let path = directory.join("libsample_ffi.so");
    std::fs::write(&path, contents).unwrap();
    path
}

#[test]
fn configured_target_wins_over_stale_default_and_preserves_profile_order() {
    if !isolated_from_ambient_target_directory("configured_target_wins_over_stale_default_and_preserves_profile_order")
    {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("workspace");
    let external = temp.path().join("external output");
    workspace(&root, &external);
    let target = RustTarget::parse("x86_64-unknown-linux-gnu").unwrap();
    artifact(&root.join("target").join(&target.triple).join("release"), b"stale");
    let native = artifact(&external.join("release"), b"native");
    let find = || find_built_artifact(&root, &target, "libsample_ffi.so", BuildProfile::Release);
    assert_eq!(find().unwrap(), native);
    let cross = artifact(&external.join(&target.triple).join("release"), b"cross");
    assert_eq!(find().unwrap(), cross);
    assert!(find_built_artifact(&root, &target, "libsample_ffi.so", BuildProfile::Debug).is_err());
    std::fs::remove_file(cross).unwrap();
    std::fs::remove_file(native).unwrap();
    let error = find().expect_err("must never stage the stale default-target library");
    assert!(error.to_string().contains(&external.display().to_string()));
}

#[test]
fn environment_target_overrides_cargo_config() {
    const CHILD: &str = "ALEF_STAGING_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        let target = RustTarget::parse("x86_64-unknown-linux-gnu").unwrap();
        let found = find_built_artifact(Path::new(&root), &target, "libsample_ffi.so", BuildProfile::Debug).unwrap();
        assert_eq!(std::fs::read(found).unwrap(), b"environment");
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("workspace");
    let configured = temp.path().join("configured");
    let external = temp.path().join("environment output");
    workspace(&root, &configured);
    artifact(&configured.join("debug"), b"wrong configured artifact");
    artifact(&root.join("target/debug"), b"stale");
    artifact(&external.join("debug"), b"environment");
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "environment_target_overrides_cargo_config", "--nocapture"])
        .env(CHILD, &root)
        .env("CARGO_TARGET_DIR", external)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

#[test]
fn stages_real_cargo_library_from_external_output_for_each_ffi_consumer() {
    if !isolated_from_ambient_target_directory("stages_real_cargo_library_from_external_output_for_each_ffi_consumer") {
        return;
    }
    use alef::core::config::{Language, NewAlefConfig};
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("workspace");
    let external = temporary.path().join("built libraries");
    workspace(&root, &external);
    let manifest = root.join("Cargo.toml");
    let mut contents = std::fs::read_to_string(&manifest).unwrap();
    contents.push_str("\n[lib]\ncrate-type = ['cdylib']\n");
    std::fs::write(&manifest, contents).unwrap();
    std::fs::write(
        root.join("mise.toml"),
        "[tasks.build]\nrun = \"cargo build --offline\"\n",
    )
    .unwrap();
    let built = mise_task::mise_run(&root, "build")
        .env("CARGO_TARGET_DIR", &external)
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let target = alef::publish::platform::host_target().unwrap();
    let filename = target.shared_lib_name("staging_probe");
    let expected = std::fs::read(external.join("debug").join(&filename)).unwrap();
    assert!(!expected.is_empty());
    let config: NewAlefConfig = toml::from_str("[workspace]\nlanguages = ['go', 'java', 'csharp']\n[[crates]]\nname = 'staging-probe'\nsources = ['src/lib.rs']\n[crates.ffi]\nlib_name = 'staging_probe'\n").unwrap();
    let resolved = config.resolve().unwrap().remove(0);
    for stale in [false, true] {
        if stale {
            let default = root.join("target/debug");
            std::fs::create_dir_all(&default).unwrap();
            std::fs::write(default.join(&filename), b"stale default library").unwrap();
        }
        for language in [Language::Go, Language::Java, Language::Csharp] {
            let staged =
                alef::publish::ffi_stage::stage_ffi(&resolved, language, &target, &root, BuildProfile::Debug).unwrap();
            assert_eq!(std::fs::read(staged).unwrap(), expected);
        }
    }
}

use super::*;

fn git(root: &Path, arguments: &[&str]) {
    let output = std::process::Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn prune(root: &Path, facade: &Path) -> String {
    git(root, &["add", facade.to_str().unwrap()]);
    successful(root, "prune", &["--", facade.to_str().unwrap()]);
    std::fs::read_to_string(facade).unwrap()
}

pub(super) fn verify(root: &Path, config: &ResolvedCrateConfig) {
    let facade = root.join(crate::backends::dart::frb_rust_facade_paths(config).unwrap().0);
    let original = std::fs::read_to_string(&facade).unwrap();
    assert_eq!(read_records(&original).unwrap().len(), 6);
    git(root, &["init", "--quiet"]);
    git(root, &["config", "core.hooksPath", ".git/hooks"]);
    git(root, &["config", "user.name", "Display probe"]);
    git(root, &["config", "user.email", "display-probe@example.test"]);
    git(root, &["add", facade.to_str().unwrap()]);
    git(
        root,
        &[
            "commit",
            "--quiet",
            "-m",
            "test: track generated facade for pruning proof",
        ],
    );
    std::fs::write(root.join("poly.toml"), "[lint.uncomment]\nenabled = true\n").unwrap();
    let unkept = original.replace("// ~keep alef-error-display:", "// alef-error-display:");
    std::fs::write(&facade, unkept).unwrap();
    let removed = prune(root, &facade);
    assert!(
        !removed.contains("alef-error-display:"),
        "uncomment negative control did no work"
    );
    successful(root, "generate", &[]);
    let bridge = root.join(crate::backends::dart::frb_dart_bridge_path(config).unwrap());
    let source = std::fs::read_to_string(&bridge).unwrap();
    let path = bridge.with_file_name("lib.freezed.dart");
    let before = std::fs::read_to_string(path).unwrap();
    assert_eq!(rewrite(&before, &source, &removed).unwrap(), before);
    rejected(root, "constructor-control", &[]);
    std::fs::write(&facade, original).unwrap();
    let preserved = prune(root, &facade);
    assert_eq!(read_records(&preserved).unwrap().len(), 6);
}

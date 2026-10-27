use super::*;

#[test]
fn generated_homebrew_suite_uses_shared_bats_assertions() {
    let tests = default_cli_tests("1.2.3");
    let suite = bats::render_suite("org/tap", "tool", None, "1.2.3", "tool", &tests);
    assert!(suite.starts_with("#!/usr/bin/env bats\n"));
    assert!(suite.contains("bats_load_library xberg-bats"));
    assert!(suite.contains("xberg_assert_status 0"));
    assert!(suite.contains("xberg_assert_output_contains '1.2.3'"));
    assert!(!suite.contains("ffi_smoke"));
    assert!(!suite.contains("PASS="));
}

#[test]
fn optional_ffi_suite_uses_per_test_temporary_directory() {
    let suite = bats::render_suite("org/tap", "tool", Some("libtool"), "1.2.3", "tool", &[]);
    assert!(suite.contains("FFI_FORMULA_QUALIFIED=\"$TAP/$FFI_FORMULA\""));
    assert!(suite.contains("pkg-config"));
    assert!(suite.contains("$BATS_TEST_TMPDIR/ffi_smoke"));
    assert!(suite.contains("run \"$BATS_TEST_TMPDIR/ffi_smoke\""));
}

#[test]
fn brewfile_and_readme_preserve_opt_in_ffi() {
    assert!(!render_brewfile("org/tap", "tool", None).contains("libtool"));
    assert!(render_brewfile("org/tap", "tool", Some("libtool")).contains("org/tap/libtool"));
    assert!(!render_readme("org/tap", "tool", None, "1.2.3", &[]).contains("ffi_smoke.c"));
    assert!(render_ffi_smoke_c("tool.h", "tool", "libtool").contains("tool_version()"));
}

#[cfg(unix)]
#[test]
fn real_bats_executes_each_command_once_and_rejects_failed_commands() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(root.join("brew"), "#!/usr/bin/env bash\nexit 0\n").unwrap();
    std::fs::set_permissions(root.join("brew"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let count = root.join("calls");
    let command = format!("echo call >> '{}'; printf 'tool 1.2.3'; exit 7", count.display());
    let cli_tests = vec![
        crate::core::config::e2e::HomebrewCliTest {
            name: "version".into(),
            command,
            expect_contains: Some("1.2.3".into()),
        },
        crate::core::config::e2e::HomebrewCliTest {
            name: "pipeline".into(),
            command: format!("echo call >> '{}'; false | printf 'tool 1.2.3'", count.display()),
            expect_contains: Some("1.2.3".into()),
        },
        crate::core::config::e2e::HomebrewCliTest {
            name: "success".into(),
            command: format!("echo call >> '{}'; printf 'tool 1.2.3'", count.display()),
            expect_contains: Some("1.2.3".into()),
        },
    ];
    let suite = root.join("homebrew.bats");
    std::fs::write(
        &suite,
        bats::render_suite("org/tap", "tool", None, "1.2.3", "tool", &cli_tests),
    )
    .unwrap();
    let output = std::process::Command::new("bats")
        .arg(&suite)
        .env("BATS_LIB_PATH", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/lib"))
        .env("PATH", format!("{}:{}", root.display(), std::env::var("PATH").unwrap()))
        .output()
        .expect("Bats must be installed for generated shell tests");
    assert!(!output.status.success(), "failing CLI must not pass on matching stdout");
    assert!(String::from_utf8_lossy(&output.stdout).contains("not ok 1 version"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("not ok 2 pipeline"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("ok 3 success"));
    assert_eq!(std::fs::read_to_string(count).unwrap().lines().count(), 3);
}

#[cfg(unix)]
fn bats_workspace() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    for (name, source) in [
        ("brew", "#!/usr/bin/env bash\nexit 0\n"),
        ("tool", "#!/usr/bin/env bash\nprintf '%s\\n' 'tool 1.2.3'\n"),
    ] {
        let path = directory.path().join(name);
        std::fs::write(&path, source).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    directory
}

#[cfg(unix)]
fn execute_suite(root: &std::path::Path, suite: &str) -> std::process::Output {
    let path = root.join("suite.bats");
    std::fs::write(&path, suite).unwrap();
    std::process::Command::new("bats")
        .arg(path)
        .env("BATS_LIB_PATH", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/lib"))
        .env("PATH", format!("{}:{}", root.display(), std::env::var("PATH").unwrap()))
        .output()
        .unwrap()
}

#[cfg(unix)]
#[test]
fn default_cli_test_checks_the_installed_version() {
    let workspace = bats_workspace();
    for (version, success) in [("1.2.3", true), ("9.9.9", false)] {
        let suite = bats::render_suite("org/tap", "tool", None, version, "tool", &default_cli_tests(version));
        let output = execute_suite(workspace.path(), &suite);
        assert_eq!(
            output.status.success(),
            success,
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1..1"));
    }
}

#[cfg(unix)]
#[test]
fn configured_commands_receive_script_directory_and_inert_literals() {
    let workspace = bats_workspace();
    let root = workspace.path();
    std::fs::write(root.join("probe"), "present").unwrap();
    let marker = root.join("injected");
    let literal = format!("literal'$(touch {})\"", marker.display());
    let command = "test -f \"$SCRIPT_DIR/probe\"; printf '%s' \"$CLI_FORMULA\"";
    let tests = vec![crate::core::config::e2e::HomebrewCliTest {
        name: "literal' title".into(),
        command: command.into(),
        expect_contains: Some(literal.clone()),
    }];
    let suite = bats::render_suite(&literal, &literal, None, &literal, "tool", &tests);
    let output = execute_suite(root, &suite);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
    assert!(!marker.exists(), "configuration literal executed as shell source");
}

#[test]
fn brewfile_keeps_ruby_interpolation_inert() {
    let literal = "value'#{system('false')}";
    let brewfile = render_brewfile(literal, literal, Some(literal));
    assert!(brewfile.contains(r"\#{system('false')}"));
}

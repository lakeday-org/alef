use super::{api_and_files, file, fixture_source, history};
use std::path::Path;

fn write(root: &Path, name: &str, content: &str) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn prepare(root: &Path) {
    for (name, content) in [
        (
            "Cargo.toml",
            include_str!("../fixtures/swift_constructor_history/core_Cargo.toml"),
        ),
        (
            "crates/fixture-ffi/Cargo.toml",
            include_str!("../fixtures/swift_constructor_history/ffi_Cargo.toml"),
        ),
        (
            "crates/fixture-ffi/src/lib.rs",
            include_str!("../fixtures/swift_constructor_history/ffi.rs"),
        ),
        (
            "mise.toml",
            include_str!("../fixtures/swift_constructor_history/mise.toml"),
        ),
    ] {
        write(root, name, content);
    }
}

fn build_stage(root: &Path, legacy: bool, arity: usize, history: &[Vec<&str>]) -> String {
    let source = fixture_source(legacy, arity, history);
    write(root, "src/lib.rs", &source);
    let (_, files) = api_and_files(&source);
    for generated in files {
        write(root, generated.path.to_str().unwrap(), &generated.content);
    }
    write(
        root,
        "packages/swift/rust/build.rs",
        include_str!("../fixtures/swift_constructor_history/build.rs"),
    );
    assert_task(root, "rust-build");
    std::fs::read_to_string(root.join("packages/swift/rust/swift-generated/Fixture/Fixture.h")).unwrap()
}

fn task(root: &Path, name: &str, variables: &[(&str, &str)]) -> alef::snippets::validators::CapturedStreams {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let mut command = super::mise_task::mise_run(root, name);
    command
        .envs(variables.iter().copied())
        .env("RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", "")
        .env("CARGO_TARGET_DIR", &target);
    let result =
        alef::snippets::validators::run_command_streams(&mut command, 180).expect("run Swift constructor fixture");
    let suffix = if variables.is_empty() { "" } else { "-negative" };
    write(
        root,
        &format!("{name}{suffix}.log"),
        &format!("{}\n{}", result.stdout, result.stderr),
    );
    result
}

fn assert_task(root: &Path, name: &str) {
    let result = task(root, name, &[]);
    assert!(result.success, "{root:?}: {name}\n{}\n{}", result.stdout, result.stderr);
}

fn declaration(header: &str, symbol: &str) -> String {
    let pattern = format!(r"(?s)[^;\n]*{}\([^;]*\);", regex::escape(symbol));
    let regex = regex::Regex::new(&pattern).unwrap();
    let declaration = regex.find(header).expect(symbol).as_str();
    declaration.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn verify_headers(root: &Path, legacy: bool) {
    let oldest = build_stage(root, legacy, 1, &[]);
    let middle_history = if legacy { vec![] } else { vec![vec!["count"]] };
    let middle = build_stage(root, legacy, 2, &middle_history);
    let current = build_stage(root, legacy, 6, &history(legacy));
    let first = "__swift_bridge__$Options$new";
    let second = if legacy {
        "__swift_bridge__$Options$new_with_fields"
    } else {
        "__swift_bridge__$Options$__alef_constructor_history_1"
    };
    assert_eq!(declaration(&oldest, first), declaration(&current, first));
    assert_eq!(declaration(&middle, second), declaration(&current, second));
    assert_eq!(declaration(&current, first).matches(',').count(), 0);
    assert_eq!(declaration(&current, second).matches(',').count(), 1);
    write(root, "frozen-oldest.h", &oldest);
    write(root, "frozen-middle.h", &middle);
    write(root, "current.h", &current);
}

fn render_probe(legacy: bool, zero: bool) -> String {
    let mut environment = minijinja::Environment::new();
    alef::template::configure_env(&mut environment);
    environment.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    environment
        .add_template(
            "probe",
            include_str!("../fixtures/swift_constructor_history/probe.swift.jinja"),
        )
        .unwrap();
    environment
        .get_template("probe")
        .unwrap()
        .render(alef::alef_context! { legacy => legacy, zero => zero })
        .unwrap()
}

fn prepare_swift(root: &Path, legacy: bool, zero: bool) {
    let generated = root.join("packages/swift/rust/swift-generated");
    let core = std::fs::read_to_string(generated.join("SwiftBridgeCore.swift")).unwrap();
    let types = std::fs::read_to_string(generated.join("Fixture/Fixture.swift")).unwrap();
    write(
        root,
        "native/RustBridge.swift",
        &["import RustBridgeC", &core, &types].join("\n"),
    );
    for (source, destination) in [
        ("SwiftBridgeCore.h", "SwiftBridgeCore.h"),
        ("Fixture/Fixture.h", "Fixture.h"),
    ] {
        write(
            root,
            &format!("native/RustBridgeC/{destination}"),
            &std::fs::read_to_string(generated.join(source)).unwrap(),
        );
    }
    write(
        root,
        "native/RustBridgeC/bridge.h",
        "#include \"SwiftBridgeCore.h\"\n#include \"Fixture.h\"\n",
    );
    write(
        root,
        "native/RustBridgeC/module.modulemap",
        "module RustBridgeC { header \"bridge.h\" export * }\n",
    );
    write(root, "native/probe.swift", &render_probe(legacy, zero));
}

fn verify_swift(root: &Path, legacy: bool, zero: bool) {
    prepare_swift(root, legacy, zero);
    for name in ["bridge-build", "facade-build", "link"] {
        assert_task(root, name);
    }
    let result = task(root, "run", &[]);
    let expected = if zero { 66 } else { 54 };
    assert!(
        result.success
            && result
                .stdout
                .contains(&format!("constructor_history_checks={expected}")),
        "{}\n{}",
        result.stdout,
        result.stderr
    );
    write(root, &format!("{expected}-checks-positive.log"), &result.stdout);
    let wrong = task(root, "run", &[("MODE", "wrong-default")]);
    assert!(
        !wrong.success && wrong.stderr.contains("Precondition failed: includeDetails"),
        "{}\n{}",
        wrong.stdout,
        wrong.stderr
    );
    let (_, missing) = api_and_files(&fixture_source(legacy, 6, &[]));
    let negative = [
        file(&missing, "Sources/Fixture/Fixture.swift"),
        "let missing: (UInt32, Bool) -> Options = Options.init(count:enabled:)",
    ]
    .join("\n");
    write(root, "native/negative.swift", &negative);
    let rejected = task(root, "negative-typecheck", &[]);
    assert!(
        !rejected.success && rejected.stderr.contains("has no member 'init(count:enabled:)'"),
        "{}\n{}",
        rejected.stdout,
        rejected.stderr
    );
}

#[test]
#[ignore = "requires Cargo, swift-bridge and native Swift linking"]
fn explicit_history_preserves_native_symbols_and_linked_function_values() {
    let mut checked = 0;
    for legacy in [true, false] {
        let directory = tempfile::Builder::new()
            .prefix("alef-swift-constructor-native-")
            .tempdir()
            .unwrap();
        let root = directory.keep();
        prepare(&root);
        verify_headers(&root, legacy);
        verify_swift(&root, legacy, false);
        checked += 1;
        if !legacy {
            let mut zero_history = history(false);
            zero_history.insert(0, vec![]);
            let header = build_stage(&root, false, 6, &zero_history);
            assert!(declaration(&header, "__swift_bridge__$Options$new").contains("(void)"));
            write(&root, "current-zero.h", &header);
            verify_swift(&root, false, true);
            checked += 1;
        }
    }
    assert_eq!(checked, 3);
}

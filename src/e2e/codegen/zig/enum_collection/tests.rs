use crate::e2e::codegen::go::test_backend::enum_collection_support as support;

#[test]
fn issue684_zig_collection_assertions_traverse_guarded_wire_lists() {
    for wire in [false, true] {
        let output = render(wire);
        assert_eq!(
            output.matches("return error.ExpectedEnumCollection").count(),
            3,
            "{output}"
        );
        assert!(!output.contains(".get(\"parts\")"), "{output}");
        assert!(!output.contains("skipped"), "{output}");
        if wire {
            assert!(output.contains(".get(\"wireText\")"), "{output}")
        }
    }
}

#[test]
#[ignore = "native Zig proof compiles generated std.json.Value assertions"]
fn issue684_zig_collection_assertions_execute_producer_backed_cases() {
    for wire in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let executable = compile_probe(wire, directory.path());
        support::verify_cases(
            support::core_cases(wire),
            [
                "PASS:3",
                "TestExpected",
                "TestExpected",
                "TestExpected",
                "ExpectedEnumCollection",
                "ExpectedEnumCollection",
            ],
            |case| {
                std::process::Command::new(&executable)
                    .env("ALEF_PROBE_INPUT", case.to_string())
                    .current_dir(directory.path())
                    .output()
                    .unwrap()
            },
        );
    }
}

fn render(wire: bool) -> String {
    let api = support::surface(wire);
    let resolver = support::resolver(&api, "zig");
    let mut output = String::new();
    for assertion in support::assertions() {
        super::super::assertions::render_json_assertion(&mut output, &assertion, "result", &resolver, false);
    }
    output
}

fn compile_probe(wire: bool, directory: &std::path::Path) -> std::path::PathBuf {
    let zig = support::tool("zig", directory);
    let source = crate::e2e::template_env::render(
        "zig/enum_collection_native_probe.zig.jinja",
        crate::alef_context! { assertions => render(wire) },
    );
    let path = directory.join("probe.zig");
    let executable = directory.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    std::fs::write(&path, source).unwrap();
    let output = std::process::Command::new(zig)
        .arg("build-exe")
        .arg("-lc")
        .arg(path)
        .arg("-O")
        .arg("Debug")
        .arg(format!("-femit-bin={}", executable.display()))
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    executable
}

use crate::core::backend::Backend;
use crate::core::config::ResolvedCrateConfig;
use crate::e2e::codegen::go::test_backend::enum_collection_support as support;

#[test]
#[ignore = "native Java proof compiles actual generated DTOs and full E2E assertions"]
fn issue688_java_collection_assertions_execute_producer_backed_cases() {
    for wire in [false, true] {
        verify_native(wire);
    }
}

fn assertions(api: &crate::core::ir::ApiSurface) -> String {
    let files = support::generate_collection_call(&super::JavaCodegen, api);
    let output = &files
        .iter()
        .find(|file| file.path.file_name().is_some_and(|name| name == "PayloadTest.java"))
        .expect("category test emitted")
        .content;
    assert!(!output.contains(".parts()"), "{output}");
    let statements = output
        .lines()
        .filter(|line| line.trim_start().starts_with("assert"))
        .collect::<Vec<_>>();
    assert_eq!(statements.len(), 11, "{output}");
    statements.join("\n")
}

fn classpath() -> String {
    let root = std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join(".m2/repository");
    let artifacts = [
        "jackson-databind",
        "jackson-core",
        "jackson-annotations",
        "junit-jupiter-api",
        "opentest4j",
        "apiguardian-api",
        "jspecify",
        "junit-platform-commons",
    ];
    let mut jars = Vec::new();
    for artifact in artifacts {
        let mut matches = walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(&format!("{artifact}-")) && name.ends_with(".jar"))
            })
            .map(|entry| entry.into_path())
            .collect::<Vec<_>>();
        matches.sort();
        jars.push(matches.pop().unwrap_or_else(|| panic!("missing cached {artifact} jar")));
    }
    std::env::join_paths(jars).unwrap().to_string_lossy().into_owned()
}

fn write_bindings(api: &crate::core::ir::ApiSurface, config: &ResolvedCrateConfig, directory: &std::path::Path) {
    let files = crate::backends::java::JavaBackend
        .generate_bindings(api, config)
        .unwrap();
    let exception = format!(
        "{}Exception.java",
        crate::backends::java::naming::main_class_name(&api.crate_name)
    );
    let mut count = 0;
    for file in files {
        let Some(name) = file.path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name != exception
            && !matches!(
                name,
                "Body.java" | "Piece.java" | "Reply.java" | "Choice.java" | "Message.java" | "JsonMapperFactory.java"
            )
        {
            continue;
        }
        let source = file
            .content
            .lines()
            .filter(|line| !line.starts_with("package "))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(directory.join(name), source).unwrap();
        count += 1;
    }
    assert_eq!(count, 7, "actual generated DTO dependencies");
}

fn verify_native(wire: bool) {
    let directory = tempfile::tempdir().unwrap();
    let api = support::call_surface(wire);
    let config = ResolvedCrateConfig {
        name: "sample".to_string(),
        ..Default::default()
    };
    write_bindings(&api, &config, directory.path());
    let source = crate::e2e::template_env::render(
        "java/list_variant_native_probe.jinja",
        crate::alef_context! { assertions => assertions(&api) },
    );
    std::fs::write(directory.path().join("Probe.java"), source).unwrap();
    let classpath = classpath();
    let javac = support::tool("javac", directory.path());
    let sources = std::fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    let output = std::process::Command::new(javac)
        .arg("-cp")
        .arg(&classpath)
        .args(sources)
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let java = support::tool("java", directory.path());
    let runtime_path = format!("{}:{}", directory.path().display(), classpath);
    support::verify_cases(
        support::core_cases(wire),
        [
            "PASS:3",
            "expected: <2>",
            "expected: <hello>",
            "expected: < world>",
            "expected list variant",
            "expected content",
        ],
        |case| {
            std::process::Command::new(&java)
                .args(["-cp", &runtime_path, "Probe", &case.to_string()])
                .current_dir(directory.path())
                .output()
                .unwrap()
        },
    );
}

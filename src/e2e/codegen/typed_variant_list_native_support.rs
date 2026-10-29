use crate::core::backend::Backend;
use crate::core::config::{NewAlefConfig, ResolvedCrateConfig};
use crate::core::ir::ApiSurface;
use std::path::{Path, PathBuf};

pub(crate) fn run(language: &str, enum_elements: bool, assertions: &str) {
    let directory = tempfile::Builder::new()
        .prefix("alef-typed-list-")
        .tempdir()
        .expect("owned native fixture");
    let path = directory.path().to_path_buf();
    let _cleanup = if std::env::var_os("ALEF_TYPED_LIST_KEEP").is_some() {
        let _ = directory.keep();
        None
    } else {
        Some(directory)
    };
    let root = path.as_path();
    let api = super::typed_variant_list_test_support::api(enum_elements, enum_elements);
    let config: NewAlefConfig =
        toml::from_str(include_str!("../../../tests/fixtures/typed_variant_list/alef.toml")).unwrap();
    let config = config.resolve().unwrap().remove(0);
    write(
        root,
        "mise.toml",
        include_str!("../../../tests/fixtures/typed_variant_list/mise.toml"),
    );
    match language {
        "csharp" => csharp(root, &api, &config, enum_elements, assertions),
        "kotlin" | "kotlin_android" => kotlin(
            root,
            &api,
            &config,
            enum_elements,
            assertions,
            language == "kotlin_android",
        ),
        "dart" => dart(root, api, &config, enum_elements, assertions),
        _ => panic!("unexpected target"),
    }
}

fn task(root: &Path, target: &str, vars: &[(&str, String)]) -> String {
    let mut command = crate::test_support::mise_task::mise_run(root, target);
    command
        .envs(vars.iter().map(|(name, value)| (name, value)))
        .env("RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", "")
        .env("CARGO_TARGET_DIR", root.join("target"));
    let output = crate::snippets::validators::run_command_streams(&mut command, 600).expect("run native mise task");
    assert!(
        output.success,
        "{root:?} {target}:\n{}\n{}",
        output.stdout, output.stderr
    );
    output.stdout
}

fn write(root: &Path, name: &str, content: &str) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn bindings(
    root: &Path,
    api: &ApiSurface,
    config: &ResolvedCrateConfig,
    backend: &dyn Backend,
    extension: &str,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for file in backend.generate_bindings(api, config).unwrap() {
        if file.path.extension().is_none_or(|value| value != extension) {
            continue;
        }
        if extension == "kt"
            && !api
                .types
                .iter()
                .any(|ty| file.path.ends_with(format!("{}.kt", ty.name)))
            && !api
                .enums
                .iter()
                .any(|ty| file.path.ends_with(format!("{}.kt", ty.name)))
        {
            continue;
        }
        let name = file.path.file_name().unwrap().to_string_lossy();
        write(root, &name, &file.content);
        paths.push(root.join(name.as_ref()));
    }
    assert!(paths.len() >= 5, "native gate must compile actual DTO and enum files");
    paths
}

fn probe(template: &str, assertions: &str, enum_elements: bool, android: bool) -> String {
    crate::e2e::template_env::render(
        template,
        crate::alef_context! {
            assertions => assertions, enum_elements => enum_elements, android => android,
        },
    )
}

fn csharp(root: &Path, api: &ApiSurface, config: &ResolvedCrateConfig, enums: bool, assertions: &str) {
    bindings(root, api, config, &crate::backends::csharp::CsharpBackend, "cs");
    write(
        root,
        "Probe.csproj",
        include_str!("../../../tests/fixtures/typed_variant_list/Probe.csproj"),
    );
    write(
        root,
        "Program.cs",
        &probe("typed_variant_list/csharp_probe.cs.jinja", assertions, enums, false),
    );
    let output = task(root, "csharp", &[]);
    assert!(output.contains("typed_list_cases=6"), "{output}");
}

fn kotlin(root: &Path, api: &ApiSurface, config: &ResolvedCrateConfig, enums: bool, assertions: &str, android: bool) {
    write(
        root,
        "pom.xml",
        include_str!("../../../tests/fixtures/typed_variant_list/pom.xml"),
    );
    task(root, "dependencies", &[]);
    let classpath = std::fs::read_to_string(root.join("classpath.txt")).unwrap();
    let mut vars = vec![
        ("JACKSON_CLASSPATH", classpath.trim().to_string()),
        ("KOTLIN_TEST_JAR", kotlin_test_jar(root).display().to_string()),
    ];
    let target = if android {
        bindings(
            root,
            api,
            config,
            &crate::backends::kotlin_android::KotlinAndroidBackend,
            "kt",
        );
        "kotlin"
    } else {
        let files = bindings(root, api, config, &crate::backends::java::JavaBackend, "java");
        vars.push((
            "JAVA_SOURCES",
            files
                .iter()
                .map(|file| file.display().to_string())
                .collect::<Vec<_>>()
                .join(" "),
        ));
        "kotlin-java"
    };
    write(
        root,
        "Main.kt",
        &probe("typed_variant_list/kotlin_probe.kt.jinja", assertions, enums, android),
    );
    let output = task(root, target, &vars);
    assert!(output.contains("typed_list_cases=6"), "{output}");
}

fn kotlin_test_jar(root: &Path) -> PathBuf {
    let compiler = which::which_in("kotlinc", std::env::var_os("PATH"), root)
        .unwrap()
        .canonicalize()
        .unwrap();
    let directory = compiler.parent().unwrap();
    [
        directory.join("../libexec/lib/kotlin-test.jar"),
        directory.join("../lib/kotlin-test.jar"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .expect("Kotlin test runtime")
}

fn dart(root: &Path, mut api: ApiSurface, config: &ResolvedCrateConfig, enums: bool, assertions: &str) {
    api.functions.push(crate::core::ir::FunctionDef {
        name: "make_sample".into(),
        rust_path: "sample::make_sample".into(),
        return_type: crate::core::ir::TypeRef::Named("Envelope".into()),
        ..Default::default()
    });
    write(
        root,
        "core/src/lib.rs",
        &crate::e2e::template_env::render(
            "typed_variant_list/core.rs.jinja",
            crate::alef_context! { enum_elements => enums, wire_rename => enums },
        ),
    );
    write(
        root,
        "core/Cargo.toml",
        include_str!("../../../tests/fixtures/typed_variant_list/core_Cargo.toml"),
    );
    write(
        root,
        "rust/Cargo.toml",
        include_str!("../../../tests/fixtures/typed_variant_list/bridge_Cargo.toml"),
    );
    write(
        root,
        "pubspec.yaml",
        include_str!("../../../tests/fixtures/typed_variant_list/pubspec.yaml"),
    );
    let files = crate::backends::dart::DartBackend
        .generate_bindings(&api, config)
        .unwrap();
    let bridge = files
        .iter()
        .find(|file| file.path.ends_with("rust/src/lib.rs"))
        .unwrap();
    write(root, "rust/src/lib.rs", &bridge.content);
    write(
        root,
        "probe_test.dart",
        &probe("typed_variant_list/dart_probe.dart.jinja", assertions, enums, false),
    );
    task(root, "dart-setup", &[]);
    task(root, "dart-generate", &[]);
    for name in ["lib.dart", "lib.freezed.dart", "frb_generated.dart"] {
        let file = root.join("lib/bridge").join(name);
        let source = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, crate::backends::dart::repair_single_variant_copy_with(&source)).unwrap();
    }
    let output = task(root, "dart", &[]);
    assert!(
        output.contains("All tests passed!") && output.contains("+6"),
        "{output}"
    );
}

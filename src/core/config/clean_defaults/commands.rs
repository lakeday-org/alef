use super::*;

pub(super) fn rust_config() -> CleanConfig {
    CleanConfig {
        precondition: Some(require_tool("cargo")),
        before: None,
        clean: Some(StringOrVec::Single("cargo clean".to_string())),
        argv_clean: None,
    }
}

pub(super) fn python_config(output_dir: &str) -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: None,
        argv_clean: Some(cd_and_run(
            output_dir,
            "rm",
            &[
                "-rf",
                "__pycache__",
                ".pytest_cache",
                ".mypy_cache",
                ".ruff_cache",
                "dist",
            ],
        )),
    }
}

pub(super) fn node_wasm_config() -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: Some(StringOrVec::Single("rm -rf node_modules dist .turbo".to_string())),
        argv_clean: None,
    }
}

pub(super) fn go_config(output_dir: &str) -> CleanConfig {
    CleanConfig {
        precondition: Some(require_tool("go")),
        before: None,
        clean: None,
        argv_clean: Some(cd_and_run(output_dir, "go", &["clean", "-cache"])),
    }
}

pub(super) fn ruby_config(output_dir: &str) -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: None,
        argv_clean: Some(cd_and_run(output_dir, "rm", &["-rf", "tmp", "vendor", ".bundle"])),
    }
}

pub(super) fn php_config(output_dir: &str) -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: None,
        argv_clean: Some(cd_and_run(output_dir, "rm", &["-rf", "vendor", "var"])),
    }
}

pub(super) fn java_config(output_dir: &str) -> CleanConfig {
    CleanConfig {
        precondition: Some(require_tool("mvn")),
        before: None,
        clean: None,
        argv_clean: Some(ArgvRunConfig {
            work_dir: ".".to_owned(),
            env: Vec::new(),
            steps: vec![ArgvStep {
                command: "mvn".to_owned(),
                args: vec![
                    "-f".to_owned(),
                    format!("{output_dir}/pom.xml"),
                    "clean".to_owned(),
                    "--batch-mode".to_owned(),
                    "--no-transfer-progress".to_owned(),
                ],
            }],
        }),
    }
}

pub(super) fn csharp_config() -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: None,
        argv_clean: None,
    }
}

pub(super) fn elixir_config(output_dir: &str) -> CleanConfig {
    CleanConfig {
        precondition: Some(require_tool("mix")),
        before: None,
        clean: None,
        argv_clean: Some(ArgvRunConfig {
            work_dir: output_dir.to_owned(),
            env: Vec::new(),
            steps: vec![
                ArgvStep {
                    command: "mix".to_owned(),
                    args: vec!["clean".to_owned()],
                },
                ArgvStep {
                    command: "rm".to_owned(),
                    args: vec!["-rf".to_owned(), "deps".to_owned(), "_build".to_owned()],
                },
            ],
        }),
    }
}

pub(super) fn r_config(output_dir: &str) -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: None,
        argv_clean: Some(cd_and_run(output_dir, "rm", &["-rf", "src/rust/target"])),
    }
}

pub(super) fn ffi_config() -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: None,
        argv_clean: None,
    }
}

pub(super) fn kotlin_config(output_dir: &str) -> CleanConfig {
    CleanConfig {
        precondition: Some(require_tool("gradle")),
        before: None,
        clean: None,
        argv_clean: Some(cd_and_run(output_dir, "gradle", &["clean"])),
    }
}

pub(super) fn kotlin_android_config(output_dir: &str) -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: None,
        argv_clean: Some(ArgvRunConfig {
            work_dir: output_dir.to_owned(),
            env: Vec::new(),
            steps: vec![ArgvStep {
                command: "./gradlew".to_owned(),
                args: vec!["clean".to_owned()],
            }],
        }),
    }
}

pub(super) fn swift_config() -> CleanConfig {
    CleanConfig {
        precondition: Some(require_tool("swift")),
        before: None,
        clean: Some(StringOrVec::Single("swift package clean".to_string())),
        argv_clean: None,
    }
}

pub(super) fn dart_config() -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: None,
        argv_clean: None,
    }
}

pub(super) fn zig_config() -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: Some(StringOrVec::Single("rm -rf zig-out zig-cache .zig-cache".to_string())),
        argv_clean: None,
    }
}

pub(super) fn gleam_config() -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: Some(StringOrVec::Single("rm -rf build".to_string())),
        argv_clean: None,
    }
}

pub(super) fn c_config() -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: Some(StringOrVec::Single("cd e2e/c && make clean".to_string())),
        argv_clean: None,
    }
}

pub(super) fn jni_config() -> CleanConfig {
    CleanConfig {
        precondition: None,
        before: None,
        clean: None,
        argv_clean: None,
    }
}

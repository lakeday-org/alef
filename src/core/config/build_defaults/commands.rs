use super::*;

pub(super) fn rust_config() -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("cargo")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single("cargo build --workspace".to_string())),
        build_release: Some(StringOrVec::Single("cargo build --release --workspace".to_string())),
        timeout_seconds: None,
    }
}

pub(super) fn python_config(crate_name: &str, ctx: &LangContext) -> BuildCommandConfig {
    {
        // Both the develop and the release command are composed here rather than written
        // out twice: the package-manager prefix and the `--features` flag are one fact each,
        // and `build_release` used to restate the whole invocation and so inherited every
        // gap `build` had. ~keep
        let feature = python_build::PYO3_EXTENSION_MODULE_FEATURE;
        let maturin = |release: bool| {
            let release_flag = if release { " --release" } else { "" };
            let command = format!(
                "maturin develop --manifest-path crates/{crate_name}-py/Cargo.toml \
                     --features {feature}{release_flag}"
            );
            python_build::run_through_python_package_manager(command, ctx.tools)
        };
        BuildCommandConfig {
            precondition: Some(require_tool(python_build::python_build_precondition_tool(ctx.tools))),
            dependency_precondition: Some(PYTHON_ENVIRONMENT_CHECK.to_string()),
            dependency_remediation: Some(python_environment_remediation(ctx.tools.python_pm())),
            before: None,
            build: Some(StringOrVec::Single(maturin(false))),
            build_release: Some(StringOrVec::Single(maturin(true))),
            timeout_seconds: None,
        }
    }
}

pub(super) fn node_config(crate_name: &str) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("npm")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        // `--package-json-path` pins napi-rs to the binding crate's own `package.json`
        // instead of letting it default to `<cwd>/package.json` (the repo root alef always
        // invokes it from), which otherwise bakes the wrong package name into the generated
        // loader whenever the repo also has a workspace-root `package.json`. See alef#368.
        build: Some(StringOrVec::Single(format!(
            "npx --yes -p @napi-rs/cli@3.7.3 napi build --manifest-path crates/{crate_name}-node/Cargo.toml -o crates/{crate_name}-node --package-json-path crates/{crate_name}-node/package.json --dts {}",
            tv::npm::NAPI_AUTO_DTS_FILENAME
        ))),
        build_release: Some(StringOrVec::Single(format!(
            "npx --yes -p @napi-rs/cli@3.7.3 napi build --manifest-path crates/{crate_name}-node/Cargo.toml -o crates/{crate_name}-node --package-json-path crates/{crate_name}-node/package.json --dts {} --release",
            tv::npm::NAPI_AUTO_DTS_FILENAME
        ))),
        timeout_seconds: None,
    }
}

pub(super) fn wasm_config(crate_name: &str) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("wasm-pack")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single(format!(
            "wasm-pack build crates/{crate_name}-wasm --dev"
        ))),
        build_release: Some(StringOrVec::Single(format!(
            "wasm-pack build crates/{crate_name}-wasm --release"
        ))),
        timeout_seconds: None,
    }
}

pub(super) fn go_config(output_dir: &str, ctx: &LangContext) -> BuildCommandConfig {
    {
        let cmd = format!("cd {output_dir} && go build ./...");
        BuildCommandConfig {
            precondition: Some(require_tool("go")),
            dependency_precondition: None,
            dependency_remediation: None,
            before: None,
            build: Some(StringOrVec::Single(wrap(cmd.clone(), ctx.run_wrapper))),
            build_release: Some(StringOrVec::Single(wrap(cmd, ctx.run_wrapper))),
            timeout_seconds: None,
        }
    }
}

pub(super) fn ruby_config(crate_name: &str) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("cargo")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single(format!("cargo build -p {crate_name}-rb"))),
        build_release: Some(StringOrVec::Single(format!("cargo build --release -p {crate_name}-rb"))),
        timeout_seconds: None,
    }
}

pub(super) fn php_config(crate_name: &str) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("cargo")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single(format!("cargo build -p {crate_name}-php"))),
        build_release: Some(StringOrVec::Single(format!(
            "cargo build --release -p {crate_name}-php"
        ))),
        timeout_seconds: None,
    }
}

pub(super) fn ffi_config(output_dir: &str) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("cargo")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single(format!(
            "cargo build --manifest-path {output_dir}/Cargo.toml"
        ))),
        build_release: Some(StringOrVec::Single(format!(
            "cargo build --release --manifest-path {output_dir}/Cargo.toml"
        ))),
        timeout_seconds: None,
    }
}

pub(super) fn java_config(output_dir: &str, ctx: &LangContext) -> BuildCommandConfig {
    {
        let (build_path, release_path) = if let Some(proj) = ctx.project_file {
            // `[crates.java] project_file` is consumer-configured and reaches `sh -c` as
            // `mvn -f`'s value -- quote it the same way `output_dir` above is quoted. ~keep
            let proj = super::super::shell::quote_word(proj);
            (
                format!("mvn -f {proj} package -DskipTests --batch-mode --no-transfer-progress"),
                format!("mvn -f {proj} package -DskipTests --batch-mode --no-transfer-progress"),
            )
        } else {
            (
                format!("mvn -f {output_dir}/pom.xml package -DskipTests --batch-mode --no-transfer-progress"),
                format!("mvn -f {output_dir}/pom.xml package -DskipTests --batch-mode --no-transfer-progress"),
            )
        };
        BuildCommandConfig {
            precondition: Some(require_tool("mvn")),
            dependency_precondition: None,
            dependency_remediation: None,
            before: None,
            build: Some(StringOrVec::Single(wrap(build_path, ctx.run_wrapper))),
            build_release: Some(StringOrVec::Single(wrap(release_path, ctx.run_wrapper))),
            timeout_seconds: None,
        }
    }
}

pub(super) fn csharp_config(output_dir: &str, ctx: &LangContext) -> BuildCommandConfig {
    {
        let (build_path, release_path) = if let Some(proj) = ctx.project_file {
            // `[crates.csharp] project_file` is consumer-configured and reaches `sh -c` as
            // `dotnet build`'s value -- quote it the same way `output_dir` above is quoted. ~keep
            let proj = super::super::shell::quote_word(proj);
            (
                format!("dotnet build {proj} --configuration Debug -q"),
                format!("dotnet build {proj} --configuration Release -q"),
            )
        } else {
            (
                format!("dotnet build {output_dir} --configuration Debug -q"),
                format!("dotnet build {output_dir} --configuration Release -q"),
            )
        };
        BuildCommandConfig {
            precondition: Some(require_tool("dotnet")),
            dependency_precondition: None,
            dependency_remediation: None,
            before: None,
            build: Some(StringOrVec::Single(wrap(build_path, ctx.run_wrapper))),
            build_release: Some(StringOrVec::Single(wrap(release_path, ctx.run_wrapper))),
            timeout_seconds: None,
        }
    }
}

pub(super) fn elixir_config(output_dir: &str) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("mix")),
        dependency_precondition: Some(mix_dependency_check(output_dir)),
        dependency_remediation: Some(format!("cd {output_dir} && mix deps.get")),
        before: None,
        build: Some(StringOrVec::Single(format!("cd {output_dir} && mix compile"))),
        build_release: Some(StringOrVec::Single(format!("cd {output_dir} && mix compile"))),
        timeout_seconds: None,
    }
}

pub(super) fn r_config(crate_name: &str) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("cargo")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single(format!("cargo build -p {crate_name}-r"))),
        build_release: Some(StringOrVec::Single(format!("cargo build --release -p {crate_name}-r"))),
        timeout_seconds: None,
    }
}

pub(super) fn kotlin_config(lang: Language, output_dir: &str, ctx: &LangContext) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("gradle")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single(wrap(
            format!("cd {output_dir} && gradle {}", gradle_build_task(lang, false)),
            ctx.run_wrapper,
        ))),
        build_release: Some(StringOrVec::Single(wrap(
            format!("cd {output_dir} && gradle {}", gradle_build_task(lang, true)),
            ctx.run_wrapper,
        ))),
        timeout_seconds: None,
    }
}

pub(super) fn kotlin_android_config(lang: Language, output_dir: &str, ctx: &LangContext) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("gradle")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single(wrap(
            format!("cd {output_dir} && gradle {}", gradle_build_task(lang, false)),
            ctx.run_wrapper,
        ))),
        build_release: Some(StringOrVec::Single(wrap(
            format!("cd {output_dir} && gradle {}", gradle_build_task(lang, true)),
            ctx.run_wrapper,
        ))),
        timeout_seconds: None,
    }
}

pub(super) fn swift_config(output_dir: &str, ctx: &LangContext) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("swift")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single(wrap(
            format!("swift build --package-path {output_dir}"),
            ctx.run_wrapper,
        ))),
        build_release: Some(StringOrVec::Single(wrap(
            format!("swift build --package-path {output_dir} --configuration release"),
            ctx.run_wrapper,
        ))),
        timeout_seconds: None,
    }
}

pub(super) fn dart_config(output_dir: &str, ctx: &LangContext) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("dart")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single(wrap(
            format!("cd {output_dir} && dart pub get"),
            ctx.run_wrapper,
        ))),
        build_release: Some(StringOrVec::Single(wrap(
            format!("cd {output_dir} && dart pub get"),
            ctx.run_wrapper,
        ))),
        timeout_seconds: None,
    }
}

pub(super) fn zig_config(output_dir: &str, ctx: &LangContext) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("zig")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single(wrap(
            format!("cd {output_dir} && zig build"),
            ctx.run_wrapper,
        ))),
        build_release: Some(StringOrVec::Single(wrap(
            format!("cd {output_dir} && zig build --release=fast"),
            ctx.run_wrapper,
        ))),
        timeout_seconds: None,
    }
}

pub(super) fn gleam_config(output_dir: &str, ctx: &LangContext) -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: Some(require_tool("gleam")),
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: Some(StringOrVec::Single(wrap(
            format!("cd {output_dir} && gleam build"),
            ctx.run_wrapper,
        ))),
        build_release: Some(StringOrVec::Single(wrap(
            format!("cd {output_dir} && gleam build"),
            ctx.run_wrapper,
        ))),
        timeout_seconds: None,
    }
}

pub(super) fn c_config() -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: None,
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: None,
        build_release: None,
        timeout_seconds: None,
    }
}

pub(super) fn jni_config() -> BuildCommandConfig {
    BuildCommandConfig {
        precondition: None,
        dependency_precondition: None,
        dependency_remediation: None,
        before: None,
        build: None,
        build_release: None,
        timeout_seconds: None,
    }
}

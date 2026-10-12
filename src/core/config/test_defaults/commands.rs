use super::*;

pub(super) fn python_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let pm = ctx.tools.python_pm();
        let (cmd, cov, pre_tool) = match pm {
            "pip" => (
                wrap(format!("cd {output_dir} && pytest"), ctx.run_wrapper),
                wrap(
                    format!("cd {output_dir} && pytest --cov=. --cov-report=lcov"),
                    ctx.run_wrapper,
                ),
                "pytest",
            ),
            "poetry" => (
                wrap(format!("cd {output_dir} && poetry run pytest"), ctx.run_wrapper),
                wrap(
                    format!("cd {output_dir} && poetry run pytest --cov=. --cov-report=lcov"),
                    ctx.run_wrapper,
                ),
                "poetry",
            ),
            _ => (
                wrap(format!("cd {output_dir} && uv run --no-sync pytest"), ctx.run_wrapper),
                wrap(
                    format!("cd {output_dir} && uv run --no-sync pytest --cov=. --cov-report=lcov"),
                    ctx.run_wrapper,
                ),
                "uv",
            ),
        };
        TestConfig {
            precondition: Some(require_tool(pre_tool)),
            before: None,
            command: Some(StringOrVec::Single(cmd)),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cov)),
        }
    }
}

pub(super) fn node_wasm_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let pm = ctx.tools.node_pm();
        let (cmd, cov) = match pm {
            "npm" => (
                wrap(format!("cd {output_dir} && npm test"), ctx.run_wrapper),
                wrap(format!("cd {output_dir} && npm test -- --coverage"), ctx.run_wrapper),
            ),
            "yarn" => (
                wrap(format!("cd {output_dir} && yarn test"), ctx.run_wrapper),
                wrap(format!("cd {output_dir} && yarn test --coverage"), ctx.run_wrapper),
            ),
            _ => (
                wrap(format!("cd {output_dir} && pnpm test"), ctx.run_wrapper),
                wrap(format!("cd {output_dir} && pnpm test -- --coverage"), ctx.run_wrapper),
            ),
        };
        TestConfig {
            precondition: Some(require_tool(pm)),
            before: None,
            command: Some(StringOrVec::Single(cmd)),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cov)),
        }
    }
}

pub(super) fn go_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let cmd = wrap(format!("cd {output_dir} && go test ./..."), ctx.run_wrapper);
        let cov = wrap(
            format!("cd {output_dir} && go test -coverprofile=coverage.out ./..."),
            ctx.run_wrapper,
        );
        TestConfig {
            precondition: Some(require_tool("go")),
            before: None,
            command: Some(StringOrVec::Single(cmd)),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cov)),
        }
    }
}

pub(super) fn ruby_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let cmd = wrap(
            format!("cd {output_dir} && {}", ruby_bundle_exec("rspec")),
            ctx.run_wrapper,
        );
        let cov = wrap(
            format!(
                "cd {output_dir} && {}",
                ruby_bundle_exec("rspec --format documentation")
            ),
            ctx.run_wrapper,
        );
        TestConfig {
            precondition: Some(require_ruby_bundler()),
            before: None,
            command: Some(StringOrVec::Single(cmd)),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cov)),
        }
    }
}

pub(super) fn php_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let cmd = wrap(format!("cd {output_dir} && composer test"), ctx.run_wrapper);
        let cov = wrap(format!("cd {output_dir} && composer test"), ctx.run_wrapper);
        TestConfig {
            precondition: Some(require_tool("composer")),
            before: None,
            command: Some(StringOrVec::Single(cmd)),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cov)),
        }
    }
}

pub(super) fn java_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let (cmd_path, cov_path) = if let Some(proj) = ctx.project_file {
            // `[crates.java] project_file` is consumer-configured and reaches `sh -c` as
            // `mvn -f`'s value -- quote it the same way `output_dir` is quoted. ~keep
            let proj = super::super::shell::quote_word(proj);
            (
                format!("mvn -f {proj} test --batch-mode --no-transfer-progress"),
                format!("mvn -f {proj} test jacoco:report --batch-mode --no-transfer-progress"),
            )
        } else {
            (
                format!("mvn -f {output_dir}/pom.xml test --batch-mode --no-transfer-progress"),
                format!("mvn -f {output_dir}/pom.xml test jacoco:report --batch-mode --no-transfer-progress"),
            )
        };
        TestConfig {
            precondition: Some(require_tool("mvn")),
            before: None,
            command: Some(StringOrVec::Single(wrap(cmd_path, ctx.run_wrapper))),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(wrap(cov_path, ctx.run_wrapper))),
        }
    }
}

pub(super) fn csharp_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let (cmd_path, cov_path) = if let Some(proj) = ctx.project_file {
            // `[crates.csharp] project_file` is consumer-configured and reaches `sh -c` as
            // `dotnet test`'s value -- quote it the same way `output_dir` is quoted. ~keep
            let proj = super::super::shell::quote_word(proj);
            (
                format!("dotnet test {proj}"),
                format!("dotnet test {proj} --collect:\"XPlat Code Coverage\""),
            )
        } else {
            (
                format!("dotnet test {output_dir}"),
                format!("dotnet test {output_dir} --collect:\"XPlat Code Coverage\""),
            )
        };
        TestConfig {
            precondition: Some(require_tool("dotnet")),
            before: None,
            command: Some(StringOrVec::Single(wrap(cmd_path, ctx.run_wrapper))),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(wrap(cov_path, ctx.run_wrapper))),
        }
    }
}

pub(super) fn elixir_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let cmd = wrap(format!("cd {output_dir} && mix test"), ctx.run_wrapper);
        let cov = wrap(format!("cd {output_dir} && mix test --cover"), ctx.run_wrapper);
        TestConfig {
            precondition: Some(require_tool("mix")),
            before: None,
            command: Some(StringOrVec::Single(cmd)),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cov)),
        }
    }
}

pub(super) fn r_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let cmd = wrap(
            format!("cd {output_dir} && Rscript -e \"testthat::test_dir('tests')\""),
            ctx.run_wrapper,
        );
        let cov = wrap(
            format!("cd {output_dir} && Rscript -e \"testthat::test_dir('tests')\""),
            ctx.run_wrapper,
        );
        TestConfig {
            precondition: Some(require_tool("Rscript")),
            before: None,
            command: Some(StringOrVec::Single(cmd)),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cov)),
        }
    }
}

pub(super) fn rust_config() -> TestConfig {
    TestConfig {
        precondition: Some(require_tool("cargo")),
        before: None,
        command: Some(StringOrVec::Single("cargo test --workspace".to_string())),
        e2e: None,
        e2e_precondition: None,
        coverage: Some(StringOrVec::Single(
            "cargo llvm-cov --workspace --lcov --output-path coverage.lcov".to_string(),
        )),
    }
}

pub(super) fn ffi_config() -> TestConfig {
    TestConfig {
        precondition: None,
        before: None,
        command: None,
        e2e: None,
        e2e_precondition: None,
        coverage: None,
    }
}

pub(super) fn c_config() -> TestConfig {
    TestConfig {
        precondition: None,
        before: None,
        command: None,
        e2e: None,
        e2e_precondition: None,
        coverage: None,
    }
}

pub(super) fn kotlin_kotlin_android_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let cmd = wrap(format!("cd {output_dir} && gradle test"), ctx.run_wrapper);
        TestConfig {
            precondition: Some(require_tool("gradle")),
            before: None,
            command: Some(StringOrVec::Single(cmd.clone())),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cmd)),
        }
    }
}

pub(super) fn swift_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let cmd = wrap(format!("swift test --package-path {output_dir}"), ctx.run_wrapper);
        TestConfig {
            precondition: Some(require_tool("swift")),
            before: None,
            command: Some(StringOrVec::Single(cmd.clone())),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cmd)),
        }
    }
}

pub(super) fn dart_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let cmd = wrap(format!("cd {output_dir} && dart test"), ctx.run_wrapper);
        let e2e_cmd = wrap(
            "cd e2e/dart && dart pub get && dart test --concurrency=1".to_string(),
            ctx.run_wrapper,
        );
        TestConfig {
            precondition: Some(require_tool("dart")),
            before: None,
            command: Some(StringOrVec::Single(cmd.clone())),
            e2e: Some(StringOrVec::Single(e2e_cmd)),
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cmd)),
        }
    }
}

pub(super) fn zig_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let cmd = wrap(format!("cd {output_dir} && zig build test"), ctx.run_wrapper);
        TestConfig {
            precondition: Some(require_tool("zig")),
            before: None,
            command: Some(StringOrVec::Single(cmd.clone())),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cmd)),
        }
    }
}

pub(super) fn gleam_config(output_dir: &str, ctx: &LangContext) -> TestConfig {
    {
        let cmd = wrap(format!("cd {output_dir} && gleam test"), ctx.run_wrapper);
        TestConfig {
            precondition: Some(require_tool("gleam")),
            before: None,
            command: Some(StringOrVec::Single(cmd.clone())),
            e2e: None,
            e2e_precondition: None,
            coverage: Some(StringOrVec::Single(cmd)),
        }
    }
}

pub(super) fn jni_config() -> TestConfig {
    TestConfig {
        precondition: None,
        before: None,
        command: None,
        e2e: None,
        e2e_precondition: None,
        coverage: None,
    }
}

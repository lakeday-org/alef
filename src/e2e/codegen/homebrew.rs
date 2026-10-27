//! Homebrew test_app generator.
//!
//! Generates a registry-mode-only test_app at `test_apps/homebrew/` that
//! exercises a Homebrew tap's CLI formula and (optionally) FFI formula.
//!
//! Emitted files:
//!
//! - `Brewfile` — declares tap + formulae for `brew bundle install`.
//! - `run_tests.sh` — compatibility launcher for Bats.
//! - `homebrew.bats` — installs via Brewfile and checks configured CLI commands.
//!   When `ffi_formula` is set it also compiles and runs `ffi_smoke.c`.
//! - `ffi_smoke.c` — minimal C program linking against the FFI formula.
//!   Only emitted when `ffi_formula` is configured.
//! - `README.md` — describes the test_app.
//!
//! This generator is registry-mode only.  In local mode it emits a single
//! stub `README.md` explaining why generation was skipped.
//!
//! # Configuration (`alef.toml`)
//!
//! ```toml
//! [crates.e2e.registry.packages.homebrew]
//! tap = "myorg/tap"
//! cli_formula = "my-tool"
//! # ffi_formula is optional; omit to suppress FFI sections entirely.
//! ffi_formula = "libmy-tool"
//!
//! # Optional: explicit CLI tests.  When absent, a single --version check is emitted.
//! [[crates.e2e.registry.packages.homebrew.cli_tests]]
//! name = "version"
//! command = "$CLI_FORMULA --version"
//! expect_contains = "1.2.3"
//! ```

mod bats;

use crate::core::backend::GeneratedFile;
use crate::core::config::ResolvedCrateConfig;
use crate::core::config::e2e::HomebrewCliTest;
use crate::core::hash::{self, CommentStyle};
use crate::e2e::config::E2eConfig;
use crate::e2e::fixture::{Fixture, FixtureGroup};
use anyhow::Result;
use std::fmt::Write as FmtWrite;
use std::path::PathBuf;

use super::E2eCodegen;
use crate::core::config::e2e::DependencyMode;

/// Homebrew formula test_app generator.
pub struct HomebrewCodegen;

impl E2eCodegen for HomebrewCodegen {
    fn generate(
        &self,
        _groups: &[FixtureGroup],
        e2e_config: &E2eConfig,
        config: &ResolvedCrateConfig,
        _type_defs: &[crate::core::ir::TypeDef],
        _enums: &[crate::core::ir::EnumDef],
        _functions: &[crate::core::ir::FunctionDef],
        _errors: &[crate::core::ir::ErrorDef],
    ) -> Result<Vec<GeneratedFile>> {
        let lang = self.language_name();
        let output_base = PathBuf::from(e2e_config.effective_output()).join(lang);

        if e2e_config.dep_mode != DependencyMode::Registry {
            return Ok(vec![GeneratedFile {
                path: output_base.join("README.md"),
                content: stub_readme(),
                generated_header: false,
            }]);
        }

        let HomebrewSettings {
            tap,
            cli_formula,
            ffi_formula,
            version,
            cli_tests,
        } = HomebrewSettings::resolve(e2e_config, config);

        let mut files = vec![
            GeneratedFile {
                path: output_base.join("Brewfile"),
                content: render_brewfile(&tap, &cli_formula, ffi_formula.as_deref()),
                generated_header: false,
            },
            GeneratedFile {
                path: output_base.join("homebrew.bats"),
                content: bats::render_suite(
                    &tap,
                    &cli_formula,
                    ffi_formula.as_deref(),
                    &version,
                    config.ffi_lib_name().as_str(),
                    &cli_tests,
                ),
                generated_header: true,
            },
            GeneratedFile {
                path: output_base.join("README.md"),
                content: render_readme(&tap, &cli_formula, ffi_formula.as_deref(), &version, &cli_tests),
                generated_header: false,
            },
        ];

        files.push(GeneratedFile {
            path: output_base.join("run_tests.sh"),
            content: crate::e2e::template_env::render("homebrew/runner.sh.jinja", crate::alef_context! {}),
            generated_header: true,
        });
        files.push(GeneratedFile {
            path: output_base.join("lib/xberg-bats/load.bash"),
            content: include_str!("../../../tests/lib/xberg-bats/load.bash").to_owned(),
            generated_header: true,
        });

        // ffi_smoke.c is only emitted when an FFI formula is configured.
        if let Some(ref ffi) = ffi_formula {
            let ffi_header = config.ffi_header_name();
            let ffi_prefix = config.ffi_prefix();
            files.push(GeneratedFile {
                path: output_base.join("ffi_smoke.c"),
                content: render_ffi_smoke_c(&ffi_header, &ffi_prefix, ffi),
                generated_header: true,
            });
        }

        Ok(files)
    }

    fn language_name(&self) -> &'static str {
        "homebrew"
    }

    /// Homebrew has no documentation-snippet recipe at all -- `render_snippet_body` is
    /// unimplemented here and falls to [`E2eCodegen::render_snippet_body`]'s own "does not
    /// support documentation snippets" error, for every fixture, always. Forwarding to it
    /// explicitly states that as a checked choice rather than an inherited default: homebrew
    /// never reaches `CallIr::signature` through this path, so it structurally cannot have the
    /// kotlin_android bug this override exists to rule out. See
    /// [`E2eCodegen::render_snippet_body_with_functions`]'s doc comment. ~keep
    fn render_snippet_body_with_functions(
        &self,
        fixture: &Fixture,
        e2e_config: &E2eConfig,
        config: &ResolvedCrateConfig,
        type_defs: &[crate::core::ir::TypeDef],
        enums: &[crate::core::ir::EnumDef],
        _functions: &[crate::core::ir::FunctionDef],
        _errors: &[crate::core::ir::ErrorDef],
    ) -> Result<String> {
        self.render_snippet_body(fixture, e2e_config, config, type_defs, enums)
    }
}

struct HomebrewSettings {
    tap: String,
    cli_formula: String,
    ffi_formula: Option<String>,
    version: String,
    cli_tests: Vec<HomebrewCliTest>,
}

impl HomebrewSettings {
    fn resolve(e2e_config: &E2eConfig, config: &ResolvedCrateConfig) -> Self {
        // Resolve Homebrew-specific package config.
        let pkg = e2e_config.registry.packages.get("homebrew");
        let tap = pkg
            .and_then(|p| p.tap.as_ref())
            .cloned()
            .unwrap_or_else(|| "example/tap".to_string());
        let cli_formula = pkg
            .and_then(|p| p.cli_formula.as_ref())
            .cloned()
            .unwrap_or_else(|| config.name.clone());
        // ffi_formula is purely opt-in: no defaulting to `lib{cli_formula}`.
        let ffi_formula: Option<String> = pkg.and_then(|p| p.ffi_formula.clone());
        // Version: prefer explicit package config, fall back to crate version, then placeholder.
        let version = pkg
            .and_then(|p| p.version.as_ref())
            .cloned()
            .or_else(|| config.resolved_version())
            .unwrap_or_else(|| "0.1.0".to_string());
        // CLI tests: use explicit config; when empty emit a single --version default.
        let cli_tests: Vec<HomebrewCliTest> = match pkg {
            Some(p) if !p.cli_tests.is_empty() => p.cli_tests.clone(),
            _ => default_cli_tests(&version),
        };

        Self {
            tap,
            cli_formula,
            ffi_formula,
            version,
            cli_tests,
        }
    }
}

/// Default CLI tests when none are configured: a single `--version` check.
fn default_cli_tests(version: &str) -> Vec<HomebrewCliTest> {
    vec![HomebrewCliTest {
        name: "version".to_string(),
        command: "$CLI_FORMULA --version".to_string(),
        expect_contains: Some(version.to_string()),
    }]
}

/// Stub README emitted in local mode.
fn stub_readme() -> String {
    "# homebrew test_app\n\nThis test_app is registry-mode only.\n\
     Run `alef e2e generate --registry` (or `alef test-apps generate`) to generate it.\n"
        .to_string()
}

/// Render `Brewfile`.
///
/// Formulae are emitted with their fully-qualified `tap/formula` names so
/// `brew bundle install` works even when other taps installed on the
/// developer's machine expose colliding formula short-names.
fn render_brewfile(tap: &str, cli_formula: &str, ffi_formula: Option<&str>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Brewfile — managed by alef. DO NOT EDIT.");
    let _ = writeln!(out, "tap {}", crate::e2e::escape::ruby_string_literal(tap));
    let formula = format!("{tap}/{cli_formula}");
    let _ = writeln!(out, "brew {}", crate::e2e::escape::ruby_string_literal(&formula));
    if let Some(ffi) = ffi_formula {
        let formula = format!("{tap}/{ffi}");
        let _ = writeln!(out, "brew {}", crate::e2e::escape::ruby_string_literal(&formula));
    }
    out
}

/// Render `ffi_smoke.c`.
///
/// The smoke test calls `{ffi_prefix}_version()`, which every FFI library
/// exposes as `const char *` — a pointer to a static version string.  It
/// does **not** return the domain-specific `{Prefix}Result` struct, so the
/// smoke C code must match that exact ABI: declare the return as
/// `const char *`, check for NULL / empty, and print without freeing (the
/// string is statically allocated inside the shared library).
fn render_ffi_smoke_c(ffi_header: &str, ffi_prefix: &str, _ffi_formula: &str) -> String {
    let mut out = String::new();
    let version_fn = format!("{ffi_prefix}_version");
    out.push_str(&hash::e2e_header(CommentStyle::Block));
    let _ = writeln!(out, "#include <{ffi_header}>");
    let _ = writeln!(out, "#include <stdio.h>");
    let _ = writeln!(out, "#include <stdlib.h>");
    let _ = writeln!(out, "#include <string.h>");
    let _ = writeln!(out);
    let _ = writeln!(out, "int main(void) {{");
    let _ = writeln!(out, "  const char *version = {version_fn}();");
    let _ = writeln!(out);
    let _ = writeln!(out, "  if (!version || strlen(version) == 0) {{");
    let _ = writeln!(
        out,
        "    fprintf(stderr, \"FAIL: expected non-empty version string\\n\");"
    );
    let _ = writeln!(out, "    return 1;");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(out);
    let _ = writeln!(out, "  printf(\"PASS: {version_fn} returned: %s\\n\", version);");
    let _ = writeln!(out, "  return 0;");
    let _ = writeln!(out, "}}");
    out
}

/// Render `README.md`.
fn render_readme(
    tap: &str,
    cli_formula: &str,
    ffi_formula: Option<&str>,
    version: &str,
    cli_tests: &[HomebrewCliTest],
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# homebrew test_app");
    out.push_str("\nRequires Bats: `brew install bats-core`.\n");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Exercises the configured Homebrew formulae from tap `{tap}` at version `{version}`."
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "| Formula | Purpose |");
    let _ = writeln!(out, "|---------|--------|");
    let _ = writeln!(out, "| `{cli_formula}` | CLI binary |");
    if let Some(ffi) = ffi_formula {
        let _ = writeln!(
            out,
            "| `{ffi}` | Shared library: C FFI for embedding in other languages |"
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## Running");
    let _ = writeln!(out);
    let _ = writeln!(out, "```bash");
    let _ = writeln!(out, "bash run_tests.sh");
    let _ = writeln!(out, "```");
    let _ = writeln!(out);
    let _ = writeln!(out, "## What it tests");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "1. `brew bundle install` succeeds (tap + formulae install without error)."
    );
    for (i, test) in cli_tests.iter().enumerate() {
        let label = if let Some(ref exp) = test.expect_contains {
            format!("`{}` — output contains `{}`", test.command, exp)
        } else {
            format!("`{}` — exits with code 0", test.command)
        };
        let _ = writeln!(out, "{}. {}.", i + 2, label);
    }
    if ffi_formula.is_some() {
        let next = cli_tests.len() + 2;
        let _ = writeln!(
            out,
            "{next}. `ffi_smoke.c` compiles against the FFI formula (via `pkg-config`) and the \
             compiled binary calls `_version()` successfully."
        );
    }
    out
}

/// Emit a test backend stub (not applicable for homebrew).
///
/// Homebrew is a packaging-verification backend with no `test_backend` stub
/// generator. Panic rather than return a placeholder `TestBackendEmission` a
/// caller could accidentally splice into generated code. ~keep
pub fn emit_test_backend(
    trait_bridge: &crate::core::config::TraitBridgeConfig,
    _methods: &[&crate::core::ir::MethodDef],
    fixture: &crate::e2e::fixture::Fixture,
) -> super::TestBackendEmission {
    panic!(
        "homebrew e2e generator: fixture `{}` requires a homebrew test_backend stub for trait `{}`, but the homebrew test-backend emitter is unimplemented; refusing to emit a call with a comment where the argument belongs",
        fixture.id, trait_bridge.trait_name
    );
}

#[cfg(test)]
#[path = "homebrew/tests.rs"]
mod tests;

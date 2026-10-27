//! Brew (Homebrew CLI) e2e test generator.
//!
//! Generates a self-contained shell-script test suite that tests a CLI binary
//! installed via Homebrew.  The suite consists of:
//!
//! - `run_tests.sh` — compatibility entry point that starts the mock server and invokes Bats.
//! - `test_{category}.bats` — one file per fixture category, each containing
//!   one Bats test per fixture.
//!
//! Each test function:
//! 1. Constructs a CLI invocation: `{binary} {subcommand} "{url}" {flags...}`
//! 2. Captures stdout into a variable.
//! 3. Uses `jq` to extract fields and checks values with native Bats assertions.
//!
//! Requirements at runtime: `bash`, `bats`, `jq`, and `MOCK_SERVER_URL` env var.

use crate::core::backend::GeneratedFile;
use crate::core::config::ResolvedCrateConfig;
use crate::e2e::config::E2eConfig;
use crate::e2e::escape::sanitize_filename;
use crate::e2e::fixture::{Fixture, FixtureGroup};
use anyhow::Result;
use std::path::PathBuf;

use super::E2eCodegen;

mod category;
mod run_tests;
mod snippet;

use category::render_category_file;
use run_tests::render_run_tests;

struct BrewSettings {
    default_subcommand: String,
    static_cli_args: Vec<String>,
    cli_flags: std::collections::HashMap<String, String>,
    binary_name: String,
}

impl BrewSettings {
    fn resolve(e2e_config: &E2eConfig) -> Self {
        // Resolve call config with overrides for the "brew" language key.
        let call = &e2e_config.call;
        let overrides = call.overrides.get("brew");
        // Default subcommand (used when fixture has no routing tags).
        let default_subcommand = overrides
            .and_then(|o| o.function.as_ref())
            .cloned()
            .unwrap_or_else(|| call.function.clone());

        // Static CLI flags appended to every invocation.
        let static_cli_args: Vec<String> = overrides.map(|o| o.cli_args.clone()).unwrap_or_default();

        // Field-to-flag mapping (fixture input field → CLI flag name).
        let cli_flags: std::collections::HashMap<String, String> =
            overrides.map(|o| o.cli_flags.clone()).unwrap_or_default();

        // Resolve binary name from the "brew" package entry, falling back to call.module.
        let binary_name = e2e_config
            .registry
            .packages
            .get("brew")
            .and_then(|p| p.name.as_ref())
            .cloned()
            .or_else(|| e2e_config.packages.get("brew").and_then(|p| p.name.as_ref()).cloned())
            .unwrap_or_else(|| call.module.clone());

        Self {
            default_subcommand,
            static_cli_args,
            cli_flags,
            binary_name,
        }
    }
}

/// Brew (Homebrew CLI) e2e code generator.
pub struct BrewCodegen;

impl E2eCodegen for BrewCodegen {
    fn generate(
        &self,
        groups: &[FixtureGroup],
        e2e_config: &E2eConfig,
        _config: &ResolvedCrateConfig,
        _type_defs: &[crate::core::ir::TypeDef],
        _enums: &[crate::core::ir::EnumDef],
        _functions: &[crate::core::ir::FunctionDef],
        // ~keep Unused on purpose: brew keeps its own single combined-output `grep -F` check
        // rather than routing through `declared_error_variant::classify`, because a CLI's error
        // text is the only observable signal it has. See that module's doc for the rationale.
        _errors: &[crate::core::ir::ErrorDef],
    ) -> Result<Vec<GeneratedFile>> {
        let lang = self.language_name();
        let output_base = PathBuf::from(e2e_config.effective_output()).join(lang);

        let BrewSettings {
            default_subcommand,
            static_cli_args,
            cli_flags,
            binary_name,
        } = BrewSettings::resolve(e2e_config);

        // Filter active groups (non-skipped fixtures).
        let active_groups: Vec<(&FixtureGroup, Vec<&Fixture>)> = groups
            .iter()
            .filter_map(|group| {
                let active: Vec<&Fixture> = group
                    .fixtures
                    .iter()
                    .filter(|f| super::should_include_fixture(f, lang, e2e_config))
                    .collect();
                if active.is_empty() { None } else { Some((group, active)) }
            })
            .collect();

        let mut files = vec![GeneratedFile {
            path: output_base.join("lib/xberg-bats/load.bash"),
            content: include_str!("../../../tests/lib/xberg-bats/load.bash").to_owned(),
            generated_header: true,
        }];

        // Generate run_tests.sh.
        let category_names: Vec<String> = active_groups
            .iter()
            .map(|(g, _)| sanitize_filename(&g.category))
            .collect();
        files.push(GeneratedFile {
            path: output_base.join("run_tests.sh"),
            content: render_run_tests(&category_names, &e2e_config.env, &binary_name, &e2e_config.alt_host),
            generated_header: true,
        });

        // Generate per-category test files.
        for (group, active) in &active_groups {
            let safe_category = sanitize_filename(&group.category);
            let filename = format!("test_{safe_category}.bats");
            let content = render_category_file(
                &group.category,
                active,
                &binary_name,
                &default_subcommand,
                &static_cli_args,
                &cli_flags,
                &e2e_config.call.args,
                e2e_config,
            );
            files.push(GeneratedFile {
                path: output_base.join(filename),
                content,
                generated_header: true,
            });
        }

        Ok(files)
    }

    fn language_name(&self) -> &'static str {
        "brew"
    }

    fn render_snippet_body(
        &self,
        fixture: &Fixture,
        e2e_config: &E2eConfig,
        _config: &ResolvedCrateConfig,
        _type_defs: &[crate::core::ir::TypeDef],
        _enums: &[crate::core::ir::EnumDef],
    ) -> Result<String> {
        snippet::render_snippet_body(fixture, e2e_config)
    }

    /// A brew snippet is a single CLI invocation (`snippet::render_snippet_body`, built from
    /// `category::build_cli_command`/`determine_subcommand`) -- it never reads a call's
    /// *result* type or renders field access, so it has no use for the free-function registry
    /// that anchors a result root, or for `errors`. Forwarding to the functions-unaware
    /// [`E2eCodegen::render_snippet_body`] here is a deliberate, checked choice rather than an
    /// inherited default -- see [`E2eCodegen::render_snippet_body_with_functions`]'s doc comment
    /// for the bug that distinction rules out. ~keep
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

/// Emit a brew test backend stub.
///
/// Brew is a packaging-verification backend with no `test_backend` stub
/// generator. Panic rather than return a placeholder `TestBackendEmission` a
/// caller could accidentally splice into generated code. ~keep
pub fn emit_test_backend(
    trait_bridge: &crate::core::config::TraitBridgeConfig,
    _methods: &[&crate::core::ir::MethodDef],
    fixture: &crate::e2e::fixture::Fixture,
) -> super::TestBackendEmission {
    panic!(
        "brew e2e generator: fixture `{}` requires a brew test_backend stub for trait `{}`, but the brew test-backend emitter is unimplemented; refusing to emit a call with a comment where the argument belongs",
        fixture.id, trait_bridge.trait_name
    );
}

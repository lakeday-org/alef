mod commands;

use super::extras::Language;
use super::output::{ArgvRunConfig, ArgvStep, CleanConfig, StringOrVec};
use super::tools::{LangContext, require_tool};

/// Build an [`ArgvRunConfig`] for a single-step clean command that `cd`s into `output_dir`
/// then runs `command` with `args`, none of it shell text.
///
/// `output_dir` is `[crates.output]`/scaffold-output-derived and therefore a free-form,
/// user-authored path -- it used to be spliced into `format!("cd {output_dir} && ...")`
/// shell strings here, which let `;`, backticks, or `$(...)` in the path execute arbitrary
/// commands (worst case: the default clean commands run `rm -rf`). `current_dir` and argv
/// arguments make it a single opaque element instead, the same fix shape as the Go test-app
/// run default. ~keep
fn cd_and_run(output_dir: &str, command: &str, args: &[&str]) -> ArgvRunConfig {
    ArgvRunConfig {
        work_dir: output_dir.to_owned(),
        env: Vec::new(),
        steps: vec![ArgvStep {
            command: command.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
        }],
    }
}

/// Return the default clean configuration for a language.
///
/// The `output_dir` is the package directory where scaffolded files live
/// (e.g. `packages/python`). It is substituted into command templates.
/// `ctx` is provided but not used; clean commands don't depend on the
/// chosen package manager.
///
/// Languages whose clean command relies only on POSIX shell builtins
/// (e.g. plain `rm -rf`) leave `precondition` as `None` since `rm` is
/// effectively always present on supported platforms.
pub(crate) fn default_clean_config(lang: Language, output_dir: &str, _ctx: &LangContext) -> CleanConfig {
    match lang {
        Language::Rust => commands::rust_config(),
        Language::Python => commands::python_config(output_dir),
        Language::Node | Language::Wasm => commands::node_wasm_config(),
        Language::Go => commands::go_config(output_dir),
        Language::Ruby => commands::ruby_config(output_dir),
        Language::Php => commands::php_config(output_dir),
        Language::Java => commands::java_config(output_dir),
        Language::Csharp => commands::csharp_config(),
        Language::Elixir => commands::elixir_config(output_dir),
        Language::R => commands::r_config(output_dir),
        Language::Ffi => commands::ffi_config(),
        Language::Kotlin => commands::kotlin_config(output_dir),
        Language::KotlinAndroid => commands::kotlin_android_config(output_dir),
        Language::Swift => commands::swift_config(),
        Language::Dart => commands::dart_config(),
        Language::Zig => commands::zig_config(),
        Language::Gleam => commands::gleam_config(),
        Language::C => commands::c_config(),
        Language::Jni => commands::jni_config(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::tools::ToolsConfig;
    use super::*;

    fn all_languages() -> Vec<Language> {
        vec![
            Language::Python,
            Language::Node,
            Language::Wasm,
            Language::Ruby,
            Language::Php,
            Language::Go,
            Language::Java,
            Language::Csharp,
            Language::Elixir,
            Language::R,
            Language::Ffi,
            Language::Rust,
            Language::Kotlin,
            Language::Swift,
            Language::Dart,
            Language::Gleam,
            Language::Zig,
        ]
    }

    fn cfg(lang: Language, dir: &str) -> CleanConfig {
        let tools = ToolsConfig::default();
        let ctx = LangContext::default(&tools);
        default_clean_config(lang, dir, &ctx)
    }

    #[test]
    fn ffi_has_no_clean_command() {
        let c = cfg(Language::Ffi, "packages/ffi");
        assert!(c.clean.is_none());
    }

    #[test]
    fn languages_with_project_agnostic_cleaning_have_defaults() {
        for lang in all_languages() {
            if matches!(lang, Language::Ffi | Language::Csharp | Language::Dart) {
                continue;
            }
            let c = cfg(lang, "packages/test");
            assert!(
                c.clean.is_some() || c.argv_clean.is_some(),
                "{lang} should have a default clean command"
            );
        }
    }

    #[test]
    fn toolchain_clean_has_precondition() {
        for lang in [Language::Rust, Language::Go, Language::Java, Language::Elixir] {
            let c = cfg(lang, "packages/test");
            let pre = c
                .precondition
                .unwrap_or_else(|| panic!("{lang} should have a precondition"));
            assert!(pre.starts_with("command -v "));
        }
    }

    #[test]
    fn pure_shell_clean_omits_precondition() {
        for lang in [
            Language::Python,
            Language::Node,
            Language::Wasm,
            Language::Ruby,
            Language::Php,
            Language::R,
        ] {
            let c = cfg(lang, "packages/test");
            assert!(
                c.precondition.is_none(),
                "{lang} pure-shell clean should not have a precondition"
            );
        }
    }

    #[test]
    fn rust_uses_cargo_clean() {
        let c = cfg(Language::Rust, "packages/rust");
        let clean = c.clean.unwrap().commands().join(" ");
        assert!(clean.contains("cargo clean"));
    }

    #[test]
    fn python_removes_pycache_and_dist() {
        let c = cfg(Language::Python, "packages/python");
        assert!(
            c.clean.is_none(),
            "python clean should be argv-only, got: {:?}",
            c.clean
        );
        let argv = c.argv_clean.expect("python should have an argv clean command");
        assert_eq!(argv.work_dir, "packages/python");
        assert_eq!(argv.steps.len(), 1);
        let step = &argv.steps[0];
        assert_eq!(step.command, "rm");
        for expected in [
            "-rf",
            "__pycache__",
            ".pytest_cache",
            ".mypy_cache",
            ".ruff_cache",
            "dist",
        ] {
            assert!(
                step.args.iter().any(|a| a == expected),
                "expected arg {expected:?} in {:?}",
                step.args
            );
        }
    }

    /// RED (pre-fix)/GREEN (post-fix): a Python package output directory containing shell
    /// metacharacters must never be reinterpreted by a shell -- it must arrive as a single,
    /// literal `current_dir`/argv element. This is the exact defect shape the Go test-app
    /// run default had (see `test_apps_run_defaults` tests): a config-supplied path spliced
    /// unquoted into `format!("cd {output_dir} && rm -rf ...")` let `;`, backticks, or
    /// `$(...)` in the path execute arbitrary commands, worst case alongside `rm -rf`.
    #[test]
    fn python_clean_treats_malicious_output_dir_as_a_single_literal_argument() {
        let malicious = "packages/python; touch pwned; echo";
        let c = cfg(Language::Python, malicious);
        assert!(
            c.clean.is_none(),
            "a shell-string `clean` here would let the payload execute; must be argv-only"
        );
        let argv = c.argv_clean.expect("python should have an argv clean command");
        assert_eq!(
            argv.work_dir, malicious,
            "the whole payload must survive verbatim as one `current_dir` value, not be shell-split"
        );
    }

    #[test]
    fn node_removes_node_modules() {
        let c = cfg(Language::Node, "packages/node");
        let clean = c.clean.unwrap().commands().join(" ");
        assert!(clean.contains("node_modules"));
        assert!(clean.contains("dist"));
    }

    #[test]
    fn wasm_matches_node() {
        let node = cfg(Language::Node, "packages/node");
        let wasm = cfg(Language::Wasm, "packages/wasm");
        assert_eq!(
            node.clean.unwrap().commands().join(" "),
            wasm.clean.unwrap().commands().join(" "),
        );
    }

    #[test]
    fn go_uses_go_clean() {
        let c = cfg(Language::Go, "packages/go");
        let argv = c.argv_clean.expect("go should have an argv clean command");
        assert_eq!(argv.work_dir, "packages/go");
        assert_eq!(argv.steps.len(), 1);
        assert_eq!(argv.steps[0].command, "go");
        assert_eq!(argv.steps[0].args, vec!["clean", "-cache"]);
    }

    #[test]
    fn java_uses_maven_clean() {
        let c = cfg(Language::Java, "packages/java");
        let argv = c.argv_clean.expect("java should have an argv clean command");
        assert_eq!(argv.steps.len(), 1);
        let step = &argv.steps[0];
        assert_eq!(step.command, "mvn");
        assert!(step.args.contains(&"clean".to_string()));
        assert!(step.args.contains(&"packages/java/pom.xml".to_string()));
    }

    #[test]
    fn csharp_requires_an_explicit_project_aware_clean_command() {
        let c = cfg(Language::Csharp, "packages/csharp");
        assert!(c.clean.is_none());
    }

    #[test]
    fn kotlin_android_uses_its_generated_gradle_wrapper() {
        let c = cfg(Language::KotlinAndroid, "packages/kotlin-android");
        assert!(
            c.precondition.is_none(),
            "the config path must not enter a shell precondition"
        );
        let argv = c.argv_clean.expect("kotlin_android should have an argv clean command");
        assert_eq!(argv.work_dir, "packages/kotlin-android");
        assert_eq!(argv.steps.len(), 1);
        assert_eq!(argv.steps[0].command, "./gradlew");
        assert_eq!(argv.steps[0].args, vec!["clean"]);
    }

    #[test]
    fn kotlin_android_clean_keeps_malicious_output_dir_out_of_shell_text() {
        let malicious = "packages/kotlin; touch /tmp/alef-kotlin-clean; #";
        let c = cfg(Language::KotlinAndroid, malicious);
        assert!(c.precondition.is_none());
        let argv = c.argv_clean.expect("kotlin_android should use argv clean");
        assert_eq!(argv.work_dir, malicious);
        assert_eq!(argv.steps[0].command, "./gradlew");
    }

    #[test]
    fn dart_requires_an_explicit_project_aware_clean_command() {
        let c = cfg(Language::Dart, "packages/dart/lib/src");
        assert!(c.clean.is_none());
    }

    #[test]
    fn elixir_uses_mix_clean() {
        let c = cfg(Language::Elixir, "packages/elixir");
        let argv = c.argv_clean.expect("elixir should have an argv clean command");
        assert_eq!(argv.work_dir, "packages/elixir");
        assert_eq!(argv.steps.len(), 2);
        assert_eq!(argv.steps[0].command, "mix");
        assert_eq!(argv.steps[0].args, vec!["clean"]);
        assert_eq!(argv.steps[1].command, "rm");
        assert_eq!(argv.steps[1].args, vec!["-rf", "deps", "_build"]);
    }

    #[test]
    fn r_removes_rust_target() {
        let c = cfg(Language::R, "packages/r");
        let argv = c.argv_clean.expect("r should have an argv clean command");
        assert_eq!(argv.steps[0].command, "rm");
        assert!(argv.steps[0].args.iter().any(|a| a == "src/rust/target"));
    }

    #[test]
    fn output_dir_substituted_in_commands() {
        let c = cfg(Language::Go, "my/custom/path");
        let argv = c.argv_clean.expect("go should have an argv clean command");
        assert_eq!(argv.work_dir, "my/custom/path");
    }
}

use super::run_run_command_with_spawn;
use anyhow::Context as _;
use serde_json::Value;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const FRB: &str = "flutter_rust_bridge_codegen";

struct RuntimeConfig {
    temporary: tempfile::TempPath,
    args: Vec<String>,
    output: PathBuf,
    dart_root: Option<PathBuf>,
}

pub(super) fn run(
    args: &[&str],
    base: &Path,
    scope: &str,
    timeout: Duration,
    before_spawn: &dyn Fn(&mut Command) -> anyhow::Result<()>,
) -> anyhow::Result<bool> {
    if std::env::var("ALEF_SKIP_COMMANDS").is_ok_and(|list| list.split(',').any(|name| name.trim() == FRB)) {
        return run_run_command_with_spawn(FRB, args, base, scope, timeout, before_spawn);
    }
    let started = Instant::now();
    let runtime = prepare(args, base)?;
    let borrowed: Vec<&str> = runtime.args.iter().map(String::as_str).collect();
    let ran = run_run_command_with_spawn(FRB, &borrowed, base, scope, remaining(started, timeout)?, before_spawn)?;
    if ran {
        complete(
            &runtime.output,
            runtime.dart_root.as_deref(),
            scope,
            started,
            timeout,
            before_spawn,
        )?;
    }
    // FRB resolves relative config paths from this file's parent; keep it alive through exit. ~keep
    drop(runtime.temporary);
    Ok(ran)
}

fn prepare(args: &[&str], base: &Path) -> anyhow::Result<RuntimeConfig> {
    let index = args
        .iter()
        .position(|arg| *arg == "--config-file")
        .context("FRB command has no --config-file")?
        + 1;
    let original = base.join(args.get(index).context("FRB command has no config filename")?);
    let parent = original
        .parent()
        .context("FRB configuration has no parent directory")?
        .canonicalize()
        .context("locating FRB configuration directory")?;
    let text = std::fs::read_to_string(&original).context("reading FRB generation configuration")?;
    let mut config: Value = serde_saphyr::from_str(&text).context("parsing FRB generation configuration")?;
    let mapping = config
        .as_object_mut()
        .context("FRB generation configuration must be a mapping")?;
    let output = parent.join(
        mapping
            .get("dart_output")
            .and_then(Value::as_str)
            .context("FRB generation configuration has no dart_output")?,
    );
    // FRB resolves an explicit relative dart_root against its process cwd, not the YAML parent. ~keep
    let dart_root = match mapping.get("dart_root") {
        None | Some(Value::Null) => None,
        Some(Value::String(path)) => Some(base.join(path)),
        Some(_) => anyhow::bail!("FRB dart_root must be a string"),
    };
    mapping.insert("build_runner".to_owned(), Value::Bool(false));
    let mut temporary = tempfile::Builder::new()
        .prefix(".alef-frb-")
        .suffix(".yaml")
        .tempfile_in(&parent)
        .context("creating private FRB configuration")?;
    // JSON is YAML-compatible and avoids rewriting or widening the shipped FRB configuration. ~keep
    serde_json::to_writer(temporary.as_file_mut(), &config).context("writing private FRB configuration")?;
    temporary.flush().context("flushing private FRB configuration")?;
    let temporary = temporary.into_temp_path();
    let mut actual: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    actual[index] = temporary
        .to_str()
        .context("FRB configuration path is not UTF-8")?
        .to_owned();
    Ok(RuntimeConfig {
        temporary,
        args: actual,
        output,
        dart_root,
    })
}

fn complete(
    output: &Path,
    dart_root: Option<&Path>,
    scope: &str,
    started: Instant,
    timeout: Duration,
    before_spawn: &dyn Fn(&mut Command) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let output = output.canonicalize().context("locating generated Dart bridge output")?;
    let bridge = output.join("lib.dart");
    let text = std::fs::read_to_string(&bridge).context("reading generated Dart bridge for part completion")?;
    let parts = generated_parts(&text)?;
    if parts.is_empty() {
        tracing::debug!("generated Dart bridge declares no parts requiring build_runner");
        return Ok(());
    }
    let (root, args) = builder_command(&output, dart_root, &parts)?;
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let ran = run_run_command_with_spawn(
        "dart",
        &borrowed,
        &root,
        scope,
        remaining(started, timeout)?,
        &|command| {
            command.env("FRB_SIMPLE_BUILD_SKIP", "1");
            before_spawn(command)
        },
    )
    .context("required Dart build_runner failed")?;
    anyhow::ensure!(ran, "required Dart build_runner was skipped or is not installed");
    for part in parts {
        anyhow::ensure!(
            output.join(&part).is_file(),
            "required generated Dart part {part} is missing after build_runner"
        );
    }
    Ok(())
}

fn builder_command(
    output: &Path,
    dart_root: Option<&Path>,
    parts: &[String],
) -> anyhow::Result<(PathBuf, Vec<String>)> {
    let root = match dart_root {
        Some(root) => root.canonicalize().context("locating explicit FRB dart_root")?,
        None => output
            .ancestors()
            .find(|parent| parent.join("pubspec.yaml").is_file())
            .context("generated Dart bridge has no package pubspec")?
            .to_path_buf(),
    };
    let pubspec: Value = serde_saphyr::from_str(&std::fs::read_to_string(root.join("pubspec.yaml"))?)
        .context("parsing generated Dart package pubspec")?;
    let name = pubspec
        .get("name")
        .and_then(Value::as_str)
        .context("generated Dart package has no name")?;
    anyhow::ensure!(
        name.bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            && !name.is_empty(),
        "generated Dart package name is invalid"
    );
    let relative = output
        .strip_prefix(root.join("lib"))
        .context("generated Dart output must be inside package lib")?;
    let prefix = escaped_filter_path(relative)?;
    let mut args = vec![
        "run".to_owned(),
        "build_runner".to_owned(),
        "build".to_owned(),
        "--delete-conflicting-outputs".to_owned(),
        format!("--build-filter=package:{name}/{prefix}**.freezed.dart"),
    ];
    if parts.iter().any(|part| part == "lib.g.dart") {
        args.push(format!("--build-filter=package:{name}/{prefix}**.g.dart"));
    }
    Ok((root.to_path_buf(), args))
}

fn generated_parts(text: &str) -> anyhow::Result<Vec<String>> {
    let regex = regex::Regex::new(r#"(?m)^\s*part\s+['\"]([^'\"]+)['\"]\s*;"#)?;
    let mut parts = Vec::new();
    for capture in regex.captures_iter(text) {
        let name = &capture[1];
        if name.ends_with(".freezed.dart") || name.ends_with(".g.dart") {
            anyhow::ensure!(
                matches!(name, "lib.freezed.dart" | "lib.g.dart"),
                "unsupported generated Dart part {name}"
            );
            if !parts.iter().any(|part| part == name) {
                parts.push(name.to_owned());
            }
        }
    }
    anyhow::ensure!(
        !parts.iter().any(|part| part == "lib.g.dart") || parts.iter().any(|part| part == "lib.freezed.dart"),
        "JSON-only generated Dart bridge is unsupported by this FRB completion path"
    );
    Ok(parts)
}

pub(super) fn escaped_filter_path(path: &Path) -> anyhow::Result<String> {
    let mut escaped = String::new();
    for component in path.components() {
        let std::path::Component::Normal(component) = component else {
            anyhow::bail!("generated Dart output filter contains a nonliteral path component");
        };
        for character in component
            .to_str()
            .context("generated Dart output filter is not UTF-8")?
            .chars()
        {
            if matches!(
                character,
                '*' | '{' | '[' | '?' | '\\' | '}' | ']' | ',' | '-' | '(' | ')'
            ) {
                escaped.push('\\');
            }
            escaped.push(character);
        }
        escaped.push('/');
    }
    let mut encoded = String::new();
    for byte in escaped.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}")?;
        }
    }
    Ok(encoded)
}

fn remaining(started: Instant, timeout: Duration) -> anyhow::Result<Duration> {
    let remaining = timeout.saturating_sub(started.elapsed());
    anyhow::ensure!(
        !remaining.is_zero(),
        "FRB generation and Dart completion exceeded their shared timeout"
    );
    Ok(remaining)
}

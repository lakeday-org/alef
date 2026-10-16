use crate::process::capture::DrainedOutput;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

const OUTPUT_LIMIT: usize = 1024 * 1024;
static NEXT_INVOCATION: AtomicU64 = AtomicU64::new(0);

pub(super) struct Observation {
    directory: PathBuf,
    started: Instant,
    preparation_ns: u128,
}

fn selected(command: &std::process::Command) -> bool {
    let program = Path::new(command.get_program())
        .file_name()
        .and_then(|name| name.to_str());
    matches!(program, Some("zig" | "zig.exe")) && command.get_args().next().is_some_and(|arg| arg == "build")
}

pub(super) fn start(command: &std::process::Command, timeout_secs: u64) -> Option<Observation> {
    let root = std::env::var_os("ALEF_ZIG_PROBE_DIR")?;
    if !selected(command) {
        return None;
    }
    match Observation::create(command, timeout_secs, Path::new(&root)) {
        Ok(observation) => Some(observation),
        Err(error) => {
            tracing::warn!(error = %error, "Zig probe could not start an observation");
            None
        }
    }
}

impl Observation {
    fn create(command: &std::process::Command, timeout_secs: u64, root: &Path) -> std::io::Result<Self> {
        let preparation_started = Instant::now();
        if !root.is_absolute() {
            return Err(std::io::Error::other("probe directory must be absolute"));
        }
        let id = NEXT_INVOCATION.fetch_add(1, Ordering::Relaxed);
        let directory = root.join(format!("{}-{id}", std::process::id()));
        std::fs::create_dir(&directory)?;
        let caches = command
            .get_envs()
            .filter(|(key, _)| *key == "ZIG_LOCAL_CACHE_DIR" || *key == "ZIG_GLOBAL_CACHE_DIR")
            .map(|(key, value)| {
                let path = value.map(Path::new);
                let empty = path.map(|path| match std::fs::read_dir(path) {
                    Ok(mut entries) => entries.next().is_none(),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
                    Err(_) => false,
                });
                (
                    key.to_string_lossy().into_owned(),
                    json!({"path":path,"initially_empty":empty}),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let metadata = json!({
            "test": std::thread::current().name().map(str::to_owned),
            "sequence": id,
            "program": command.get_program().to_string_lossy(),
            "args": command.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect::<Vec<_>>(),
            "cwd": command.get_current_dir(),
            "timeout_secs": timeout_secs,
            "utc_unix_ms": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map_err(std::io::Error::other)?.as_millis(),
            "caches": caches,
        });
        std::fs::write(directory.join("start.tmp"), serde_json::to_vec_pretty(&metadata)?)?;
        std::fs::rename(directory.join("start.tmp"), directory.join("start.json"))?;
        if let Some(build_file) = command.get_args().skip_while(|arg| *arg != "--build-file").nth(1) {
            let source = Path::new(build_file)
                .parent()
                .ok_or_else(|| std::io::Error::other("build root missing"))?;
            for name in ["build.zig", "build.zig.zon", "snippet.zig"] {
                let path = source.join(name);
                if path.is_file() {
                    std::fs::copy(path, directory.join(name))?;
                }
            }
        }
        Ok(Self {
            directory,
            preparation_ns: preparation_started.elapsed().as_nanos(),
            started: Instant::now(),
        })
    }

    pub(super) fn finish(&self, verdict: &str, output: std::result::Result<&DrainedOutput, &std::io::Error>) {
        if let Err(error) = self.write_finish(verdict, output) {
            tracing::warn!(error = %error, "Zig probe could not finish an observation");
        }
    }

    fn write_finish(
        &self,
        verdict: &str,
        output: std::result::Result<&DrainedOutput, &std::io::Error>,
    ) -> std::io::Result<()> {
        let finish_started = Instant::now();
        let mut metadata = json!({
            "verdict":verdict,"child_and_drain_elapsed_ms":self.started.elapsed().as_millis(),
            "pre_spawn_observation_ns":self.preparation_ns,
        });
        match output {
            Ok(output) => {
                for (name, text) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
                    let bytes = text.as_bytes();
                    std::fs::write(
                        self.directory.join(format!("{name}.txt")),
                        &bytes[..bytes.len().min(OUTPUT_LIMIT)],
                    )?;
                    metadata[name] = json!({"bytes":bytes.len(),"saved_bytes":bytes.len().min(OUTPUT_LIMIT)});
                }
                metadata["drain_complete"] = json!(output.complete);
            }
            Err(error) => metadata["drain_error"] = json!(error.to_string()),
        }
        metadata["finish_output_io_ns"] = json!(finish_started.elapsed().as_nanos());
        metadata["timing_scope"] = json!("preparation and output-file IO measured; final finish.json write excluded");
        std::fs::write(
            self.directory.join("finish.json"),
            serde_json::to_vec_pretty(&metadata)?,
        )
    }
}

#[test]
fn should_observe_only_real_zig_build_commands() {
    for (program, argument, expected) in [
        ("zig", "build", true),
        ("zig.exe", "build", true),
        ("zig", "version", false),
        ("other", "build", false),
    ] {
        let mut command = std::process::Command::new(program);
        command.arg(argument);
        assert_eq!(selected(&command), expected);
    }
}

#[test]
fn should_preserve_timeout_output_and_report_truncation() {
    let root = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new("zig");
    command
        .arg("build")
        .env("ZIG_LOCAL_CACHE_DIR", root.path().join("cold-cache"));
    let observation = Observation::create(&command, 300, root.path()).unwrap();
    let output = Ok(DrainedOutput {
        stdout: "marker-stdout".into(),
        stderr: "x".repeat(OUTPUT_LIMIT + 1),
        complete: true,
    });
    observation.write_finish("timeout", output.as_ref()).unwrap();
    assert_eq!(
        std::fs::read(observation.directory.join("stdout.txt")).unwrap(),
        b"marker-stdout"
    );
    assert_eq!(
        std::fs::metadata(observation.directory.join("stderr.txt"))
            .unwrap()
            .len(),
        OUTPUT_LIMIT as u64
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(observation.directory.join("finish.json")).unwrap()).unwrap();
    assert_eq!(metadata["verdict"], "timeout");
    assert_eq!(metadata["stderr"]["bytes"], OUTPUT_LIMIT + 1);
    assert_eq!(metadata["stderr"]["saved_bytes"], OUTPUT_LIMIT);
    assert_eq!(metadata["drain_complete"], true);
    assert!(metadata["pre_spawn_observation_ns"].as_u64().is_some());
    assert!(metadata["finish_output_io_ns"].as_u64().is_some());
    assert_eq!(
        metadata["timing_scope"],
        "preparation and output-file IO measured; final finish.json write excluded"
    );
}

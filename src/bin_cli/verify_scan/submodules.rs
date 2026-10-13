use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub(super) fn paths(base: &Path) -> HashSet<PathBuf> {
    let mut paths = HashSet::new();
    if base.join(".gitmodules").is_file()
        && let Some(output) = git_output(
            base,
            &[
                "config",
                "--file",
                ".gitmodules",
                "--null",
                "--get-regexp",
                r"^submodule\..*\.path$",
            ],
        )
    {
        for record in output.split(|&byte| byte == 0) {
            if let Some(index) = record.iter().position(|&byte| byte == b'\n') {
                insert_relative(&mut paths, &record[index + 1..]);
            }
        }
    }
    if let Some(output) = git_output(base, &["ls-files", "--stage", "-z", "--", "."]) {
        for record in output.split(|&byte| byte == 0) {
            if record.starts_with(b"160000 ")
                && let Some(index) = record.iter().position(|&byte| byte == b'\t')
            {
                insert_relative(&mut paths, &record[index + 1..]);
            }
        }
    }
    paths
}

fn git_output(base: &Path, args: &[&str]) -> Option<Vec<u8>> {
    match Command::new("git").arg("-C").arg(base).args(args).output() {
        Ok(output) if output.status.success() => Some(output.stdout),
        Ok(output) => {
            tracing::debug!(path = %base.display(), operation = args[0], status = %output.status, "submodule metadata query unavailable; retaining filesystem scan");
            None
        }
        Err(error) => {
            tracing::warn!(path = %base.display(), operation = args[0], %error, "reading submodule metadata failed; retaining filesystem scan");
            None
        }
    }
}

fn insert_relative(paths: &mut HashSet<PathBuf>, bytes: &[u8]) {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    let path = Path::new(text);
    if !text.is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
    {
        paths.insert(path.components().filter(|part| *part != Component::CurDir).collect());
    }
}

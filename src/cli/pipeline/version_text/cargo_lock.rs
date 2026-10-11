/// Rewrite the `version = "..."` field of every local/path-source `[[package]]`
/// entry in a committed `Cargo.lock` so it matches the freshly-bumped manifests.
///
/// A binding that ships a committed `Cargo.lock` inside its source tarball (e.g.
/// a Rustler NIF crate packaged into a Hex release) must keep that lockfile in
/// step with the workspace version, otherwise `cargo build` from the published
/// tarball fails with a lock/manifest version mismatch.
///
/// Registry dependencies carry a `source = "registry+..."` (or `git+...`) key
/// and an upstream-pinned version that must never be rewritten. Local crates —
/// the consumer's own workspace members and the NIF crate itself — have NO
/// `source` key, but can have independent versions. Resolve each from its manifest,
/// preserving registry/git entries and local packages excluded from discovery. ~keep
///
/// The lockfile is line-oriented and `cargo` rewrites it deterministically, so a
/// targeted line rewrite (rather than a full TOML re-serialize) preserves the
/// canonical formatting and avoids reordering. Returns the rewritten content
/// when at least one local entry changed, else `None`.
pub(in crate::cli::pipeline) fn sync_cargo_lock_path_versions(
    content: &str,
    versions: &std::collections::HashMap<String, String>,
) -> Option<String> {
    let mut out = String::with_capacity(content.len());
    let mut changed = false;

    let mut block: Vec<&str> = Vec::new();
    let mut in_package_block = false;

    for line in content.lines() {
        if line.trim() == "[[package]]" {
            changed |= append_package_block(&mut block, &mut out, versions);
            in_package_block = true;
            block.push(line);
        } else if in_package_block {
            block.push(line);
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    changed |= append_package_block(&mut block, &mut out, versions);

    if !changed {
        return None;
    }
    if !content.ends_with('\n') {
        out.pop();
    }
    Some(out)
}

fn append_package_block(
    block: &mut Vec<&str>,
    out: &mut String,
    versions: &std::collections::HashMap<String, String>,
) -> bool {
    if block.is_empty() {
        return false;
    }
    let mut changed = false;
    let is_package = block.first().is_some_and(|l| l.trim() == "[[package]]");
    let has_source = block.iter().any(|l| l.trim_start().starts_with("source = "));
    let version = block.iter().find_map(|line| {
        line.trim()
            .strip_prefix("name = \"")
            .and_then(|name| name.strip_suffix('"'))
            .and_then(|name| versions.get(name))
    });
    for line in block.iter() {
        if is_package
            && !has_source
            && line.trim_start().starts_with("version = ")
            && let Some(new_version) = version
        {
            let indent_len = line.len() - line.trim_start().len();
            let indent = &line[..indent_len];
            let rewritten = format!(r#"{indent}version = "{new_version}""#);
            if rewritten != *line {
                changed = true;
            }
            out.push_str(&rewritten);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    block.clear();
    changed
}

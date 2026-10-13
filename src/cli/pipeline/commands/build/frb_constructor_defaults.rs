use anyhow::Context as _;
use std::path::Path;

pub(super) fn apply(
    source: &str,
    base_dir: &Path,
    config: &crate::core::config::ResolvedCrateConfig,
) -> anyhow::Result<String> {
    let Some((path, _)) = crate::backends::dart::frb_rust_facade_paths(config) else {
        return Ok(source.into());
    };
    let path = base_dir.join(path);
    let mirror = std::fs::read_to_string(&path)
        .with_context(|| format!("reading FRB constructor metadata from {}", path.display()))?;
    crate::backends::dart::apply_constructor_defaults(source, &mirror)
}

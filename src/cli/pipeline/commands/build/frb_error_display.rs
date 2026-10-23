use anyhow::Context as _;
use std::path::Path;

pub(super) fn apply(
    source: &str,
    base_dir: &Path,
    config: &crate::core::config::ResolvedCrateConfig,
) -> anyhow::Result<String> {
    let Some((facade, _)) = crate::backends::dart::frb_rust_facade_paths(config) else {
        return Ok(source.into());
    };
    let path = base_dir.join(facade);
    if !path.exists() {
        return Ok(source.into());
    }
    let mirror = std::fs::read_to_string(&path)
        .with_context(|| format!("reading FRB error display metadata from {}", path.display()))?;
    if !mirror.contains("// ~keep alef-error-display: ") {
        return Ok(source.into());
    }
    let bridge = base_dir
        .join(crate::backends::dart::frb_dart_bridge_path(config).context("locating Dart error factory declarations")?);
    let bridge = std::fs::read_to_string(&bridge)
        .with_context(|| format!("reading Dart error factory declarations from {}", bridge.display()))?;
    crate::backends::dart::error_display::rewrite(source, &bridge, &mirror)
}

#[cfg(test)]
mod tests {
    use crate::core::backend::PostProcessor;
    use crate::core::config::NewAlefConfig;

    const BRIDGE: &str =
        "sealed class SampleError { const factory SampleError.rejected({required String message}) = BoundError; }";
    const FREEZED: &str = "class BoundError extends SampleError {\n const BoundError({required this.message});\n final String message;\n String toString() { return 'old'; }\n}";
    const META: &str = r#"// ~keep alef-error-display: {"owner":"SampleError","factory":"rejected","fields":1,"chunks":[{"text":"rejected: "},{"field":0}]}"#;

    #[test]
    fn existing_freezed_pipeline_applies_metadata_and_rejects_missing_factories() {
        let root = tempfile::tempdir().unwrap();
        let config: NewAlefConfig = toml::from_str(
            "[workspace]\nlanguages=[\"dart\"]\n[[crates]]\nname=\"sample\"\nsources=[\"src/lib.rs\"]\n",
        )
        .unwrap();
        let config = config.resolve().unwrap().remove(0);
        let facade = root
            .path()
            .join(crate::backends::dart::frb_rust_facade_paths(&config).unwrap().0);
        let bridge = root
            .path()
            .join(crate::backends::dart::frb_dart_bridge_path(&config).unwrap());
        for path in [&facade, &bridge] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        }
        std::fs::write(facade, META).unwrap();
        std::fs::write(&bridge, BRIDGE).unwrap();
        let target = bridge.with_file_name("lib.freezed.dart");
        std::fs::write(&target, FREEZED).unwrap();
        let processor = PostProcessor::FrbDartSingleVariantCopyWith;
        super::super::post_process::run(&target, root.path(), &processor, &config).unwrap();
        let rewritten = std::fs::read_to_string(&target).unwrap();
        assert!(rewritten.contains("message.toString()"));
        super::super::post_process::run(&target, root.path(), &processor, &config).unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), rewritten);
        std::fs::write(&bridge, "no factories").unwrap();
        let error = super::super::post_process::run(&target, root.path(), &processor, &config).unwrap_err();
        assert!(format!("{error:#}").contains("expected one FRB error factory, found 0"));
    }

    #[test]
    fn legacy_processor_without_metadata_retains_original_behavior() {
        let root = tempfile::tempdir().unwrap();
        let config: NewAlefConfig = toml::from_str(
            "[workspace]\nlanguages=[\"dart\"]\n[[crates]]\nname=\"sample\"\nsources=[\"src/lib.rs\"]\n",
        )
        .unwrap();
        let config = config.resolve().unwrap().remove(0);
        assert_eq!(super::apply(FREEZED, root.path(), &config).unwrap(), FREEZED);
    }
}

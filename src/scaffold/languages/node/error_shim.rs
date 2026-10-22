use anyhow::Context as _;
use std::path::Path;

const STALE_INDEX_EXPORTS: [(&str, &str); 2] = [
    (
        "module.exports = nativeBinding;",
        "module.exports = require(\"./binding.cjs\")(nativeBinding);",
    ),
    (
        "module.exports = { ...nativeBinding, ..._service };",
        "module.exports = { ...require(\"./binding.cjs\")(nativeBinding), ..._service };",
    ),
];

fn export_fragment(entrypoint: &str, include_import: bool) -> String {
    crate::scaffold::template_env::render(
        "node_error_shim_export.json.jinja",
        crate::alef_context! { entrypoint => entrypoint, include_import => include_import },
    )
    .trim_end_matches('\n')
    .to_string()
}

fn add_shim_files_to_files_array(package_json: &str) -> Option<String> {
    let files_start = package_json.find("\"files\"")?;
    let array_start = files_start + package_json[files_start..].find('[')?;
    let array_end = array_start + package_json[array_start..].find(']')?;
    let array = &package_json[array_start..array_end];
    if array.contains("\"binding.cjs\"") || array.contains("\"index.mjs\"") {
        return None;
    }
    let entry = array.find("\"index.js\"")?;
    let after_entry = &array[entry + "\"index.js\"".len()..];
    let replacement = if let Some(rest) = after_entry.strip_prefix(",\n") {
        let indent: String = rest.chars().take_while(|c| c.is_whitespace() && *c != '\n').collect();
        crate::scaffold::template_env::render(
            "node_error_shim_files.json.jinja",
            crate::alef_context! { indent => indent },
        )
        .trim_end_matches('\n')
        .to_string()
    } else {
        "\"index.js\", \"binding.cjs\", \"index.mjs\"".to_string()
    };
    let mut migrated = String::with_capacity(package_json.len() + replacement.len());
    migrated.push_str(&package_json[..array_start + entry]);
    migrated.push_str(&replacement);
    migrated.push_str(&package_json[array_start + entry + "\"index.js\"".len()..]);
    Some(migrated)
}

fn updated_index(index: &str) -> Option<String> {
    if index.contains("binding.cjs") {
        return Some(index.to_string());
    }
    let (stale, fixed) = STALE_INDEX_EXPORTS
        .iter()
        .find(|(stale, _)| index.matches(stale).count() == 1)?;
    Some(index.replacen(stale, fixed, 1))
}

fn updated_package(package: &str) -> Option<String> {
    let mut migrated = package.to_string();
    if !package.contains("\"./index.mjs\"") {
        let (entrypoint, stale) = ["index.js", "index-wrapper.cjs"].into_iter().find_map(|entrypoint| {
            let stale = export_fragment(entrypoint, false);
            (package.matches(&stale).count() == 1).then_some((entrypoint, stale))
        })?;
        migrated = package.replacen(&stale, &export_fragment(entrypoint, true), 1);
    }
    Some(add_shim_files_to_files_array(&migrated).unwrap_or(migrated))
}

fn persist_repair(path: &Path, content: &str) -> anyhow::Result<()> {
    let parent = path.parent().context("node package path has no parent directory")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary file in {}", parent.display()))?;
    std::io::Write::write_all(&mut temporary, content.as_bytes())
        .with_context(|| format!("failed to write temporary file for {}", path.display()))?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

/// Patch only recognized scaffold shapes after both generated shim files exist. ~keep
pub(crate) fn migrate_node_error_shim(base_dir: &Path, relative_path: &Path) -> anyhow::Result<bool> {
    let package_json_path = crate::cli::pipeline::generate::write::contained_output_path(base_dir, relative_path)?;
    let Some(crate_root) = package_json_path.parent() else {
        return Ok(false);
    };
    if !crate_root.join("binding.cjs").is_file() || !crate_root.join("index.mjs").is_file() {
        return Ok(false);
    }
    let index_path = crate_root.join("index.js");
    let (Ok(package_json), Ok(index_js)) = (
        std::fs::read_to_string(&package_json_path),
        std::fs::read_to_string(&index_path),
    ) else {
        return Ok(false);
    };
    let (Some(migrated_index), Some(migrated_package)) = (updated_index(&index_js), updated_package(&package_json))
    else {
        return Ok(false);
    };
    if migrated_index == index_js && migrated_package == package_json {
        return Ok(false);
    }
    persist_repair(&index_path, &migrated_index)?;
    persist_repair(&package_json_path, &migrated_package)?;
    tracing::info!(
        path = %package_json_path.display(),
        "repaired pre-existing crates/*-node package: routed index.js through binding.cjs and added the ESM entry"
    );
    Ok(true)
}

#[cfg(test)]
mod error_shim_migration_tests {
    use super::*;

    const OLD_PACKAGE_JSON: &str = "{\n  \"name\": \"@scope/example\",\n  \"main\": \"index.js\",\n  \"exports\": {\n    \".\": {\n      \"types\": \"./index.d.ts\",\n      \"require\": \"./index.js\",\n      \"default\": \"./index.js\"\n    }\n  },\n  \"files\": [\n    \"index.js\",\n    \"index.d.ts\",\n    \"*.node\"\n  ]\n}\n";
    const OLD_INDEX_JS: &str = "\"use strict\";\nlet nativeBinding = null;\nmodule.exports = nativeBinding;\n";

    fn package_dir(with_shim_files: bool) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("crates/example-node");
        std::fs::create_dir_all(&root).expect("create crate dir");
        std::fs::write(root.join("package.json"), OLD_PACKAGE_JSON).expect("write package.json");
        std::fs::write(root.join("index.js"), OLD_INDEX_JS).expect("write index.js");
        if with_shim_files {
            std::fs::write(root.join("binding.cjs"), "").expect("write binding.cjs");
            std::fs::write(root.join("index.mjs"), "").expect("write index.mjs");
        }
        dir
    }

    #[test]
    fn routes_index_js_through_the_shim_and_adds_the_esm_entry() {
        let dir = package_dir(true);
        let relative = Path::new("crates/example-node/package.json");
        assert!(migrate_node_error_shim(dir.path(), relative).expect("migration runs"));

        let root = dir.path().join("crates/example-node");
        let index = std::fs::read_to_string(root.join("index.js")).unwrap();
        assert!(
            index.contains("module.exports = require(\"./binding.cjs\")(nativeBinding);"),
            "{index}"
        );
        let package = std::fs::read_to_string(root.join("package.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&package).expect("package.json stays valid JSON");
        assert_eq!(parsed["exports"]["."]["import"], "./index.mjs", "{package}");
        assert_eq!(parsed["exports"]["."]["require"], "./index.js", "{package}");
        let files: Vec<&str> = parsed["files"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(files, ["index.js", "binding.cjs", "index.mjs", "index.d.ts", "*.node"]);

        assert!(
            !migrate_node_error_shim(dir.path(), relative).expect("second run"),
            "the migration must be idempotent"
        );
    }

    #[test]
    fn waits_until_the_shim_files_exist() {
        let dir = package_dir(false);
        let relative = Path::new("crates/example-node/package.json");
        assert!(!migrate_node_error_shim(dir.path(), relative).expect("migration runs"));
        let root = dir.path().join("crates/example-node");
        assert_eq!(std::fs::read_to_string(root.join("index.js")).unwrap(), OLD_INDEX_JS);
        assert_eq!(
            std::fs::read_to_string(root.join("package.json")).unwrap(),
            OLD_PACKAGE_JSON
        );
    }

    #[test]
    fn leaves_a_hand_edited_index_js_alone() {
        let dir = package_dir(true);
        let root = dir.path().join("crates/example-node");
        std::fs::write(root.join("index.js"), "module.exports = { custom: true };\n").unwrap();
        let relative = Path::new("crates/example-node/package.json");
        assert!(!migrate_node_error_shim(dir.path(), relative).expect("migration runs"));
        assert_eq!(
            std::fs::read_to_string(root.join("package.json")).unwrap(),
            OLD_PACKAGE_JSON
        );
    }
}

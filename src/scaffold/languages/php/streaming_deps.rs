use crate::core::config::{AdapterPattern, ResolvedCrateConfig};
use crate::core::template_versions as tv;

pub(super) fn append_stream_dependencies(config: &ResolvedCrateConfig, dependencies: &mut String) {
    let streaming: Vec<_> = config
        .adapters
        .iter()
        .filter(|adapter| {
            matches!(adapter.pattern, AdapterPattern::Streaming)
                && !adapter.skip_languages.iter().any(|language| language == "php")
        })
        .collect();
    if streaming.is_empty() {
        return;
    }
    let needs_anyhow = streaming.iter().any(|adapter| {
        adapter
            .error_type
            .as_deref()
            .is_none_or(|name| name.trim_start_matches("::") == "anyhow::Error")
    });
    for (name, version, needed) in [
        ("futures-util", tv::cargo::FUTURES_UTIL, true),
        ("anyhow", tv::cargo::ANYHOW, needs_anyhow),
    ] {
        if !needed || crate::scaffold::cargo_dependency_declared(dependencies.lines(), name) {
            continue;
        }
        if !dependencies.is_empty() {
            dependencies.push('\n');
        }
        dependencies.push_str(&format!("{name} = \"{version}\""));
    }
}

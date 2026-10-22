use super::*;

#[test]
fn streaming_parameters_keep_builtin_and_external_type_paths_resolvable() {
    let mut executed = 0;
    for language in [Language::Php, Language::Ruby] {
        let mut config = make_config(vec![language]);
        let mut adapter = worker_stack::streaming_adapter();
        adapter.params = [
            ("limit", "u32", false),
            ("optional", "u32", true),
            ("label", "String", false),
            ("request", "Request", false),
            ("event", "external::Event", false),
            ("unit", "()", false),
        ]
        .into_iter()
        .map(|(name, ty, optional)| AdapterParam {
            name: name.into(),
            ty: ty.into(),
            optional,
        })
        .collect();
        config.adapters.push(adapter);
        let bodies = build_adapter_bodies(&config, language).unwrap();
        let bindings: Vec<_> = bodies["DataClient.stream_data"]
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("let core_"))
            .collect();
        assert_eq!(bindings.len(), 6, "{language}: {bindings:?}");
        let bindings = bindings.join("\n");
        assert!(bindings.contains("core_limit: u32"), "{bindings}");
        assert!(bindings.contains("core_request: test_crate::Request"), "{bindings}");
        assert!(bindings.contains("core_event: external::Event"), "{bindings}");
        assert!(bindings.contains("core_unit: ()"), "{bindings}");
        executed += check_bindings_compile(&bindings);
    }
    assert_eq!(executed, 4);
}

fn check_bindings_compile(bindings: &str) -> usize {
    let directory = tempfile::tempdir().unwrap();
    let mut executed = 0;
    for valid in [false, true] {
        let bindings = if valid {
            bindings.to_string()
        } else {
            let broken = bindings.replace("core_limit: u32", "core_limit: test_crate::u32");
            assert_ne!(broken, bindings);
            broken
        };
        let source = format!(
            "mod test_crate {{ #[derive(Clone)] pub struct Request; }}\n\
             mod external {{ #[derive(Clone)] pub struct Event; }}\n\
             fn main() {{ let limit = 1_u32; let optional = Some(2_u32);\n\
             let label = String::new(); let request = test_crate::Request;\n\
             let event = external::Event; let unit = ();\n\
             {bindings}\n\
             let _ = (core_limit, core_optional, core_label, core_request, core_event, core_unit); }}"
        );
        let file = directory.path().join("probe.rs");
        std::fs::write(&file, source).unwrap();
        let result = std::process::Command::new("rustc")
            .args(["--edition=2024", "--emit=metadata"])
            .arg(&file)
            .arg("-o")
            .arg(directory.path().join("probe.rmeta"))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(result.status.success(), valid, "{stderr}");
        if !valid {
            assert!(
                stderr.contains("cannot find type `u32` in module `test_crate`"),
                "{stderr}"
            );
        }
        executed += 1;
    }
    executed
}

use anyhow::Context;

pub(crate) fn receiver_call(
    core_path: &str,
    receiver: &str,
    borrowed_receiver: &str,
    args: &str,
) -> anyhow::Result<String> {
    let path: syn::ExprPath =
        syn::parse_str(core_path).with_context(|| format!("parsing adapter Rust callable `{core_path}`"))?;
    let qualified = path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() > 1;
    Ok(if qualified {
        let separator = if args.is_empty() { "" } else { ", " };
        format!("{core_path}({borrowed_receiver}{separator}{args})")
    } else {
        format!("{receiver}.{core_path}({args})")
    })
}

#[cfg(test)]
mod tests {
    use super::receiver_call;

    #[test]
    fn receiver_calls_preserve_rust_path_syntax_and_reject_expressions() {
        let cases = [
            ("events", "inner.events(request)"),
            ("events::<Vec<u8>>", "inner.events::<Vec<u8>>(request)"),
            ("r#type", "inner.r#type(request)"),
            ("sample::Engine::events", "sample::Engine::events(&inner, request)"),
            (
                "<Engine as Events>::events",
                "<Engine as Events>::events(&inner, request)",
            ),
            (
                "::sample::events::<Vec<u8>>",
                "::sample::events::<Vec<u8>>(&inner, request)",
            ),
        ];
        assert_eq!(cases.len(), 6);
        for (path, expected) in cases {
            let call = receiver_call(path, "inner", "&inner", "request").unwrap();
            assert_eq!(call, expected);
            syn::parse_str::<syn::Expr>(&call).expect("actual call is valid Rust syntax");
        }
        assert_eq!(
            receiver_call("sample::events", "inner", "inner", "").unwrap(),
            "sample::events(inner)"
        );
        for path in ["", "events()", "events; panic!()", "sample::"] {
            let error = receiver_call(path, "inner", "&inner", "").unwrap_err();
            assert!(error.to_string().contains("parsing adapter Rust callable"));
            assert!(error.root_cause().is::<syn::Error>());
        }
    }

    #[test]
    fn receiver_call_native_dispatch_preserves_trait_helpers_and_generics() {
        let calls = [
            ("events", "inner", "&inner", "", 1),
            ("Engine::events", "inner", "&inner", "", 1),
            ("<Engine as Events>::events", "inner", "&inner", "", 2),
            ("helper", "inner", "&inner", "", 3),
            ("helpers::events", "inner", "&inner", "", 4),
            ("generic::<Vec<u8>>", "inner", "&inner", "", 5),
            ("Engine::generic::<Vec<u8>>", "inner", "&inner", "", 5),
            ("r#type", "inner", "&inner", "", 6),
            ("Engine::events", "borrowed", "borrowed", "", 1),
        ];
        assert_eq!(calls.len(), 9);
        let assertions = calls
            .into_iter()
            .map(|(path, receiver, borrowed, args, expected)| {
                let call = receiver_call(path, receiver, borrowed, args).unwrap();
                format!("assert_eq!({call}, {expected});")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let source = include_str!("rust_callable_probe.rs.txt").replace("__ASSERTIONS__", &assertions);
        check_dispatch_controls(&source);
    }

    fn check_dispatch_controls(source: &str) {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("Taskfile.yml"), "version: '3'\ntasks:\n  proof:\n    cmds:\n      - rustc --edition=2024 probe.rs -o probe\n      - ./probe\n").unwrap();
        for valid in [false, true] {
            let candidate = if valid {
                source.to_string()
            } else {
                source.replace("<Engine as Events>::events(&inner)", "inner.events()")
            };
            std::fs::write(directory.path().join("probe.rs"), candidate).unwrap();
            let output = std::process::Command::new("task")
                .args(["proof"])
                .current_dir(directory.path())
                .env("RUSTC_WRAPPER", "")
                .env("RUSTC_WORKSPACE_WRAPPER", "")
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.success(), valid, "{stderr}");
            if !valid {
                assert!(stderr.contains("left: 1") && stderr.contains("right: 2"), "{stderr}");
            }
        }
    }
}

use quote::ToTokens;

pub(crate) fn awaited_core_call(source: &str) -> String {
    struct Calls(Vec<String>);
    impl<'ast> syn::visit::Visit<'ast> for Calls {
        fn visit_expr_await(&mut self, expression: &'ast syn::ExprAwait) {
            self.0.push(expression.base.to_token_stream().to_string());
            syn::visit::visit_expr_await(self, expression);
        }
    }
    let parsed = syn::parse_file(source).expect("parse actual emitted Rust");
    let mut calls = Calls(Vec::new());
    syn::visit::Visit::visit_file(&mut calls, &parsed);
    calls
        .0
        .into_iter()
        .find(|call| call.contains("events") || call.contains("generic") || call.contains("r#type"))
        .expect("actual emitted core invocation must be awaited")
}

pub(crate) fn verify_dispatch(calls: &[(String, u32)], tuple_receiver: bool) {
    assert!(!calls.is_empty(), "native proof must exercise emitted invocations");
    let checks = calls
        .iter()
        .map(|(call, expected)| {
            let client = if tuple_receiver {
                "TupleClient(Engine)"
            } else {
                "Wrapper { inner: Engine }"
            };
            format!(
                concat!(
                    "{{ let inner = Engine; let client = {client}; let client_ref = &inner; ",
                    "let resource = Wrapper {{ inner: Engine }}; let __alef_req = 5u32; ",
                    "let core_req = 5u32; let request = 5u32; let req_owned = 5u32; let req = 5u32; ",
                    "assert_eq!({call}.await.unwrap(), {expected}); }}"
                ),
                client = client,
                call = call,
                expected = expected,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let source = include_str!("callable_native_probe.rs.txt").replace("__CHECKS__", &checks);
    let directory = tempfile::tempdir().unwrap();
    let executable = format!("probe{}", std::env::consts::EXE_SUFFIX);
    let taskfile = format!(
        concat!(
            "version: '3'\ntasks:\n  proof:\n    cmds:\n",
            "      - rustc --edition=2024 probe.rs -o {executable}\n      - ./{executable}\n"
        ),
        executable = executable,
    );
    std::fs::write(directory.path().join("Taskfile.yml"), taskfile).unwrap();
    check_dispatch_controls(&source, directory.path());
}

fn check_dispatch_controls(source: &str, directory: &std::path::Path) {
    for valid in [false, true] {
        let candidate = if valid {
            source.to_owned()
        } else {
            assert!(
                source.contains("< Engine as Events > :: events"),
                "negative must target trait dispatch"
            );
            source.replace("< Engine as Events > :: events", "Engine::events")
        };
        std::fs::write(directory.join("probe.rs"), candidate).unwrap();
        let result = std::process::Command::new("task")
            .arg("proof")
            .current_dir(directory)
            .env("RUSTC_WRAPPER", "")
            .env("RUSTC_WORKSPACE_WRAPPER", "")
            .output()
            .unwrap();
        let error = String::from_utf8_lossy(&result.stderr);
        assert_eq!(result.status.success(), valid, "{error}");
        if !valid {
            assert!(error.contains("left: 6") && error.contains("right: 7"), "{error}");
        }
    }
}

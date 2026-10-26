use super::*;
use crate::e2e::codegen::c::streaming::content::ContentProbe;

fn renamed_types(explicit: bool) -> Vec<TypeDef> {
    let mut types = chunk_types(optional_text());
    types.extend([
        TypeDef {
            name: "StreamToolCall".into(),
            fields: vec![field("function", named("StreamFunction"))],
            ..Default::default()
        },
        TypeDef {
            name: "StreamFunction".into(),
            fields: vec![field("name", TypeRef::String)],
            ..Default::default()
        },
    ]);
    for owner in &mut types {
        owner.serde_rename_all = Some("camelCase".into());
        if explicit {
            for field in &mut owner.fields {
                field.serde_rename = Some(format!("wire_{}", field.name));
            }
        }
    }
    types
}

fn fixture() -> Fixture {
    Fixture {
        assertions: vec![
            field_assertion("finish_reason", "equals", Some(serde_json::json!("stop"))),
            field_assertion("tool_calls", "count_equals", Some(serde_json::json!(1))),
            field_assertion(
                "tool_calls[0].function.name",
                "equals",
                Some(serde_json::json!("lookup")),
            ),
        ],
        ..Default::default()
    }
}

fn render(explicit: bool) -> (String, String, String) {
    render_types(&renamed_types(explicit))
}

fn render_types(types: &[TypeDef]) -> (String, String, String) {
    let fixture = fixture();
    assert_eq!(fixture.assertions.len(), 3);
    let probe = ContentProbe::resolve(&fixture, types, "StreamChunk");
    let (mut declarations, mut capture, mut assertions) = Default::default();
    probe.render_declarations(&mut declarations);
    probe.render_capture(&mut capture, "sample", "chunk", "Sample");
    for assertion in &fixture.assertions {
        assert!(probe.render_assertion(&mut assertions, assertion));
    }
    probe.render_release(&mut assertions);
    (declarations, capture, assertions)
}

#[test]
fn json_keys_follow_owner_casing_and_explicit_rename_priority() {
    for (explicit, keys) in [
        (false, ["finishReason", "delta", "toolCalls", "function", "name"]),
        (
            true,
            [
                "wire_finish_reason",
                "wire_delta",
                "wire_tool_calls",
                "wire_function",
                "wire_name",
            ],
        ),
    ] {
        let (_, capture, _) = render(explicit);
        for key in keys {
            assert!(capture.contains(&format!(", \"{key}\")")), "{capture}");
        }
        assert!(capture.contains("sample_stream_chunk_choices(chunk)"), "{capture}");
        assert!(!capture.contains(", \"finish_reason\")"), "{capture}");
    }
}

#[test]
#[ignore = "requires a C compiler"]
fn native_json_capture_honors_wire_names_and_rejects_old_literals() {
    use crate::e2e::codegen::streaming_assertions::probe_support::Probe;
    let native = Probe::new("c");
    for explicit in [false, true] {
        let (declarations, capture, assertions) = render(explicit);
        let payload = if explicit {
            r#"[{"wire_finish_reason":"stop","wire_delta":{"wire_tool_calls":[{"wire_function":{"wire_name":"lookup"}}]}}]"#
        } else {
            r#"[{"finishReason":"stop","delta":{"toolCalls":[{"function":{"name":"lookup"}}]}}]"#
        };
        for (old_literals, success) in [(false, true), (true, false)] {
            let capture = if old_literals {
                capture
                    .replace("finishReason", "finish_reason")
                    .replace("wire_finish_reason", "finish_reason")
            } else {
                capture.clone()
            };
            native_status(&native, &declarations, &capture, &assertions, payload, success);
        }
    }
}

fn native_status(
    native: &crate::e2e::codegen::streaming_assertions::probe_support::Probe,
    declarations: &str,
    capture: &str,
    assertions: &str,
    payload: &str,
    success: bool,
) {
    let source = crate::e2e::template_env::render(
        "c/content_runtime_probe.jinja",
        crate::alef_context! {
            header => crate::e2e::codegen::c::runner::render_test_runner_header(&[], &[]),
            declarations => declarations, capture => capture, assertions => assertions,
            payload => crate::e2e::escape::escape_c(payload),
        },
    );
    let output = native.run(&source);
    crate::e2e::codegen::streaming_assertions::probe_support::assert_status(&output, success);
    if !success {
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains("assertion failed:"), "{diagnostic}");
        assert!(diagnostic.contains("finish_reason"), "{diagnostic}");
    }
}

fn escaped_types(suffix: &str) -> Vec<TypeDef> {
    let mut types = renamed_types(true);
    for owner in &mut types {
        for field in &mut owner.fields {
            field.serde_rename = Some(format!("wire_{}{suffix}", field.name));
        }
    }
    types
}

fn escaped_payload(suffix: &str, wrong_finish: bool) -> String {
    let finish = if wrong_finish {
        "wrong_finish".into()
    } else {
        format!("wire_finish_reason{suffix}")
    };
    serde_json::json!([{
        (finish): "stop",
        (format!("wire_delta{suffix}")): {
            (format!("wire_tool_calls{suffix}")): [{
                (format!("wire_function{suffix}")): {(format!("wire_name{suffix}")): "lookup"}
            }]
        }
    }])
    .to_string()
}

#[test]
fn json_key_content_is_encoded_before_c_literal_escaping() {
    for suffix in ["\"", "\\", "\n"] {
        let (_, capture, _) = render_types(&escaped_types(suffix));
        for name in ["finish_reason", "delta", "tool_calls", "function", "name"] {
            let json = serde_json::to_string(&format!("wire_{name}{suffix}")).expect("JSON key");
            let encoded = crate::e2e::escape::escape_c(&json[1..json.len() - 1]);
            assert!(capture.contains(&format!(", \"{encoded}\")")), "{capture}");
        }
    }
}

#[test]
#[ignore = "requires a C compiler"]
fn native_escaped_json_keys_match_and_wrong_keys_fail() {
    use crate::e2e::codegen::streaming_assertions::probe_support::Probe;
    let native = Probe::new("c");
    let mut cases = 0;
    for suffix in ["\"", "\\", "\n"] {
        let (declarations, capture, assertions) = render_types(&escaped_types(suffix));
        for (wrong_finish, success) in [(false, true), (true, false)] {
            native_status(
                &native,
                &declarations,
                &capture,
                &assertions,
                &escaped_payload(suffix, wrong_finish),
                success,
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 6);
}

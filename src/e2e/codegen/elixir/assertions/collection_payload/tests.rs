use crate::e2e::collection_payload_test_support as support;
use std::collections::{HashMap, HashSet};

fn rendered() -> String {
    let mut output = String::new();
    for assertion in support::assertions() {
        super::super::render_assertion(
            &mut output,
            &assertion,
            "result",
            &support::resolver(false),
            "Sample",
            &HashSet::new(),
            &HashMap::new(),
            false,
            false,
            false,
            false,
            true,
        );
    }
    output
}

fn producer() -> String {
    assert_eq!(support::wire_value()["content"].as_array().unwrap().len(), 2);
    let api = support::api();
    let piece = api.enums.iter().find(|item| item.name == "Piece").unwrap();
    assert!(!crate::backends::rustler::gen_bindings::helpers::is_flat_data_enum(
        piece
    ));
    let body = api.enums.iter().find(|item| item.name == "Body").unwrap();
    assert!(crate::backends::rustler::gen_bindings::helpers::is_flat_data_enum(body));
    let module = crate::backends::rustler::gen_bindings::helpers::gen_elixir_enum_module_with_known_types(
        piece,
        "Sample",
        &Default::default(),
        "sample_core",
        None,
    );
    assert!(
        module.contains("def text(text), do: {:text, %{text: text}}"),
        "{module}"
    );
    module
}

#[test]
fn elixir_collection_payload_uses_actual_tagged_tuple_fields() {
    let output = rendered();
    assert_eq!(output.matches("assert ").count(), 3, "{output}");
    assert!(!output.contains("skipped:"), "{output}");
    assert!(output.contains("Enum.at(result.content.parts, 0)"), "{output}");
    assert!(output.contains("Enum.at(result.content.parts, 1)"), "{output}");
    assert_eq!(output.matches("Map.fetch!(payload, :text)").count(), 2, "{output}");
    assert!(!producer().is_empty());
}

#[test]
fn elixir_collection_payload_executes_generated_constructors_and_assertions() {
    let directory = tempfile::tempdir().unwrap();
    let Ok(elixir) = which::which_in("elixir", std::env::var_os("PATH"), directory.path()) else {
        assert!(std::env::var_os("ALEF_REQUIRE_ELIXIR").is_none(), "Elixir required");
        return;
    };
    let source = crate::e2e::template_env::render(
        "elixir/collection_payload_probe.jinja",
        crate::alef_context! { producer => producer(), assertions => rendered() },
    );
    let path = directory.path().join("probe.exs");
    std::fs::write(&path, source).unwrap();
    let output = std::process::Command::new(elixir)
        .current_dir(directory.path())
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "verified 7 Elixir payload cases and old tuple failure"
    );
}

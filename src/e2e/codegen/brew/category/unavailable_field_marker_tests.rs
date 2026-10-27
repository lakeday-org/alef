use super::render_test_function;
use crate::e2e::config::{CallConfig, E2eConfig};
use crate::e2e::fixture::{Assertion, Fixture};

fn e2e_config_with_result_fields() -> E2eConfig {
    E2eConfig {
        result_fields: std::collections::HashSet::from(["content".to_string()]),
        call: CallConfig {
            function: "process".to_string(),
            ..CallConfig::default()
        },
        ..E2eConfig::default()
    }
}

fn dirty_fixture(id: &str) -> Fixture {
    Fixture {
        id: id.to_string(),
        description: "test fixture".to_string(),
        assertions: vec![Assertion {
            assertion_type: "equals".to_string(),
            field: Some("nonexistent_field".to_string()),
            value: Some(serde_json::json!("x")),
            ..Default::default()
        }],
        ..Fixture::default()
    }
}

/// A field absent from `result_fields` renders the shell skip-comment; the
/// wired-in `fail_on_unavailable_field_markers` call must see that exact
/// text in the fixture's own rendered body.
#[test]
fn dropped_field_assertion_carries_the_marker() {
    let e2e_config = e2e_config_with_result_fields();
    let mut out = String::new();

    render_test_function(
        &mut out,
        &dirty_fixture("dirty_smoke"),
        "mycli",
        "extract",
        &[],
        &std::collections::HashMap::new(),
        &[],
        &e2e_config,
    );

    assert!(
        out.contains("# skipped: field 'nonexistent_field' not available on result type"),
        "got: {out}"
    );
}

/// Two fixtures rendered into the same shared `out` buffer (as
/// `render_category_file` does): the first fixture's render must not pick up
/// a skip marker appended later by the second fixture, proving the scan is
/// correctly offset-scoped per fixture rather than scanning the whole buffer.
#[test]
fn dropped_field_assertion_is_correctly_attributed_per_fixture() {
    let e2e_config = e2e_config_with_result_fields();
    let mut out = String::new();

    let clean_fixture = Fixture {
        id: "clean_smoke".to_string(),
        description: "test fixture".to_string(),
        assertions: vec![Assertion {
            assertion_type: "not_error".to_string(),
            ..Default::default()
        }],
        ..Fixture::default()
    };
    render_test_function(
        &mut out,
        &clean_fixture,
        "mycli",
        "extract",
        &[],
        &std::collections::HashMap::new(),
        &[],
        &e2e_config,
    );
    let clean_len = out.len();

    render_test_function(
        &mut out,
        &dirty_fixture("dirty_smoke"),
        "mycli",
        "extract",
        &[],
        &std::collections::HashMap::new(),
        &[],
        &e2e_config,
    );

    assert!(
        !out[..clean_len].contains("not available"),
        "the first fixture's own render must carry no skip marker, got:\n{}",
        &out[..clean_len]
    );
    assert!(
        out[clean_len..].contains("# skipped: field 'nonexistent_field' not available on result type"),
        "got:\n{}",
        &out[clean_len..]
    );
}

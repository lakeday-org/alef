use crate::core::ir::ErrorDef;
use crate::e2e::fixture::Fixture;

pub(crate) fn emit_runtime(out: &mut String, fixture: &Fixture, prefix: &str, language: &str) {
    emit_runtime_with_errors(out, fixture, prefix, language, &[]);
}

pub(crate) fn emit_runtime_with_errors(
    out: &mut String,
    fixture: &Fixture,
    prefix: &str,
    language: &str,
    errors: &[ErrorDef],
) {
    let markers = super::render_with_errors(fixture, prefix, language, errors);
    let warning = super::super::inert_example::coverage_warning(&markers, language, &fixture.id);
    out.push_str(&markers);
    out.push_str(&warning);
}

/// ~keep Some callers receive an already-closed method; diagnostics belong inside its runtime body.
pub(crate) fn emit_completed_runtime(
    out: &mut String,
    fixture: &Fixture,
    prefix: &str,
    language: &str,
    errors: &[ErrorDef],
    closing: &str,
) {
    let markers = super::render_with_errors(fixture, prefix, language, errors);
    let warning = super::super::inert_example::coverage_warning(&markers, language, &fixture.id);
    if !warning.is_empty() {
        let end = out.rfind(closing).expect("generated test closes its runtime body");
        out.insert_str(end + 1, &warning);
    }
    out.push_str(&markers);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e2e::fixture::Assertion;

    fn fixture() -> Fixture {
        Fixture {
            id: "error_probe".into(),
            assertions: vec![
                Assertion {
                    assertion_type: "error".into(),
                    ..Default::default()
                },
                Assertion {
                    assertion_type: "equals".into(),
                    field: Some("unavailable".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn completed_method_diagnostics_execute_before_each_language_closer() {
        for (language, closing) in [
            ("gleam", "\n}"),
            ("elixir", "\n  end"),
            ("r", "\n})"),
            ("dart", "\n  });"),
        ] {
            let mut out = format!("method body{closing}\n");
            emit_completed_runtime(&mut out, &fixture(), "// ", language, &[], closing);
            let warning = out.find("missing assertion coverage").expect("warning emitted");
            assert!(
                warning < out.find(closing).expect("closer retained"),
                "{language}: {out}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "generated test closes its runtime body")]
    fn missing_runtime_closer_is_not_a_zero_work_pass() {
        emit_completed_runtime(&mut String::new(), &fixture(), "// ", "dart", &[], "\n  });");
    }
}

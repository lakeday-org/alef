/// ~keep Legacy markers lack `skipped:`; diagnostic copies start with `alef` and are not ledger entries.
pub(crate) fn is_skip_marker_line(line: &str) -> bool {
    super::COMMENT_OPENERS.iter().any(|opener| {
        line.trim_start()
            .strip_prefix(opener)
            .is_some_and(|body| !body.trim_start().starts_with("alef "))
    })
}

pub(super) fn visible_control_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_control() {
            out.extend(character.escape_default());
        } else {
            out.push(character);
        }
    }
    out
}

pub(super) fn skip_details(body: &str) -> Vec<String> {
    body.lines()
        .filter(|line| {
            super::COMMENT_OPENERS
                .iter()
                .any(|opener| line.trim_start().starts_with(opener))
        })
        .filter_map(|line| {
            line.split_once("skipped: ")
                .map(|(_, reason)| reason.trim().trim_end_matches(" */").to_owned())
        })
        .collect()
}

/// ~keep Warnings run after real assertions: a skip can hide nonfatal XCTest/xUnit failures.
pub(crate) fn coverage_warning(body: &str, language: &str, fixture_id: &str) -> String {
    let details = skip_details(body);
    if details.is_empty() {
        return String::new();
    }
    let reason = visible_control_text(&format!(
        "alef missing assertion coverage for fixture `{fixture_id}`: {} assertion(s) skipped: {}",
        details.len(),
        details.join("; "),
    ));
    let literal = if matches!(language, "ruby" | "php") {
        format!("'{}'", reason.replace('\\', "\\\\").replace('\'', "\\'"))
    } else {
        let quoted = serde_json::to_string(&reason).expect("diagnostic string serializes");
        if language.starts_with("kotlin") || language == "dart" {
            quoted.replace('$', "\\$")
        } else if language == "elixir" {
            quoted.replace("#{", "\\#{")
        } else {
            quoted
        }
    };
    crate::e2e::template_env::render(
        "coverage_warning.jinja",
        crate::alef_context! {
            language => language, reason => literal,
        },
    )
}

/// ~keep Complete C/Rust bodies include their closing brace; diagnostics must stay inside it.
pub(crate) fn completed_body(body: &str, language: &str, fixture_id: &str) -> String {
    let warning = coverage_warning(body, language, fixture_id);
    let mut out = body.to_owned();
    if !warning.is_empty() {
        let end = out.rfind("\n}").expect("generated test function closes its body");
        out.insert_str(end + 1, &warning);
    }
    out
}

pub(crate) fn warn_completed_test(out: &mut String, start: usize, fixture_id: &str) {
    let body = completed_body(&out[start..], "c", fixture_id);
    out.truncate(start);
    out.push_str(&body);
}

pub(crate) fn coverage_warning_for_path(
    body: &str,
    expects_error: bool,
    error_bodies: &[&str],
    language: &str,
    fixture_id: &str,
) -> String {
    if expects_error {
        coverage_warning(&error_bodies.join("\n"), language, fixture_id)
    } else {
        coverage_warning(body, language, fixture_id)
    }
}

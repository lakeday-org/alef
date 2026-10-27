mod value_capture;

use crate::core::hash::{self, CommentStyle};
use crate::e2e::codegen::field_skip::FieldSkip;
use crate::e2e::config::E2eConfig;
use crate::e2e::escape::{escape_shell, sanitize_ident};
use crate::e2e::field_access::FieldResolver;
use crate::e2e::fixture::{Assertion, Fixture};
use std::fmt::Write as FmtWrite;

/// Render a per-category `test_{category}.sh` file.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_category_file(
    category: &str,
    fixtures: &[&Fixture],
    binary_name: &str,
    default_subcommand: &str,
    static_cli_args: &[String],
    cli_flags: &std::collections::HashMap<String, String>,
    args: &[crate::e2e::config::ArgMapping],
    e2e_config: &E2eConfig,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "#!/usr/bin/env bats");
    out.push_str(&hash::e2e_header(CommentStyle::Hash));
    let _ = writeln!(out, "# E2e tests for category: {category}");
    out.push_str("setup() {\n  bats_load_library xberg-bats\n  set -euo pipefail\n}\n");
    let _ = writeln!(out);

    for fixture in fixtures {
        render_test_function(
            &mut out,
            fixture,
            binary_name,
            default_subcommand,
            static_cli_args,
            cli_flags,
            args,
            e2e_config,
        );
        let _ = writeln!(out);
    }

    out
}

/// Render the shell body for an `error`-asserting test's command execution.
///
/// ~keep With no declared value this returns output byte-identical to the
/// pre-existing "fail if the command succeeds" check. When a value is
/// declared, brew has no typed-exception hierarchy to check separately from
/// the message — the CLI's combined stdout/stderr is the only observable
/// signal, and error-type names (e.g. "BadRequest") are typically printed
/// into that same text by the CLI's own error formatting. So a single
/// substring search over the captured output serves the message-or-type
/// disjunction other backends implement as two checks (see
/// `declared_error_value`'s doc comment). `grep -F` performs a literal
/// substring search rather than a regex match, so no regex-metacharacter
/// escaping is needed for the search itself; the value still needs shell
/// quoting since it lands inside the generated script.
fn render_error_test_body(cmd: &str, declared_error: Option<&str>) -> String {
    let mut out = String::new();
    match declared_error {
        Some(value) => {
            let quoted = format!("'{}'", escape_shell(value));
            let _ = writeln!(out, "  if output=$({cmd} 2>&1); then");
            let _ = writeln!(
                out,
                "    echo 'FAIL [error]: expected command to fail but it succeeded' >&2"
            );
            let _ = writeln!(out, "    return 1");
            let _ = writeln!(out, "  fi");
            let _ = writeln!(out, "  if ! grep -qF {quoted} <<<\"$output\"; then");
            // `quoted` is already a complete, self-contained single-quoted shell
            // fragment (from `escape_shell`). Embedding it inside a *double*-quoted
            // echo string here would both break on a literal `"` in the value and,
            // worse, re-enable `$`/backtick expansion of the value's raw content
            // inside double quotes. Concatenate it as an adjacent single-quoted
            // segment instead — bash joins adjacent quoted words with no operator —
            // which stays injection-safe regardless of what the value contains.
            let _ = writeln!(out, "    echo 'FAIL [error]: expected output to contain '{quoted} >&2");
            let _ = writeln!(out, "    return 1");
            let _ = write!(out, "  fi");
        }
        None => {
            let _ = writeln!(out, "  if {cmd} >/dev/null 2>&1; then");
            let _ = writeln!(
                out,
                "    echo 'FAIL [error]: expected command to fail but it succeeded' >&2"
            );
            let _ = writeln!(out, "    return 1");
            let _ = write!(out, "  fi");
        }
    }
    out
}

/// Render a single `test_{id}()` function for a fixture.
#[allow(clippy::too_many_arguments)]
fn render_test_function(
    out: &mut String,
    fixture: &Fixture,
    binary_name: &str,
    default_subcommand: &str,
    static_cli_args: &[String],
    cli_flags: &std::collections::HashMap<String, String>,
    _args: &[crate::e2e::config::ArgMapping],
    e2e_config: &E2eConfig,
) {
    let fn_name = sanitize_ident(&fixture.id);
    let description = &fixture.description;

    let expects_error = fixture.assertions.iter().any(|a| a.assertion_type == "error");

    out.push_str(&crate::e2e::template_env::render(
        "brew/bats_test_header.jinja",
        crate::alef_context! { name => fn_name },
    ));
    let _ = writeln!(out, "  # {description}");

    // Resolve fixture-specific call config if provided, otherwise use defaults.
    let call_config = e2e_config.resolve_call_for_fixture(
        fixture.call.as_deref(),
        &fixture.id,
        &fixture.resolved_category(),
        &fixture.tags,
        &fixture.input,
    );

    // Check if this call is unsupported in brew. If so, skip the test with a documented reason.
    if let Some(reason) = call_config.unsupported_in.get("brew") {
        let _ = writeln!(out, "  # SKIP [brew unsupported]: {reason}");
        let _ = writeln!(out, "  skip {}", crate::core::config::shell::quote_word(reason));
        let _ = writeln!(out, "}}");
        return;
    }
    let call_field_resolver = FieldResolver::new(
        e2e_config.effective_fields(call_config),
        e2e_config.effective_fields_optional(call_config),
        e2e_config.effective_result_fields(call_config),
        e2e_config.effective_fields_array(call_config),
        &std::collections::HashSet::new(),
    );
    let field_resolver = &call_field_resolver;

    // Determine subcommand: authoritative from brew override function if present,
    // otherwise fall back to tag-derived determination.
    let subcommand = if let Some(brew_override) = call_config.overrides.get("brew") {
        if let Some(override_fn) = &brew_override.function {
            override_fn.clone()
        } else {
            determine_subcommand(&fixture.tags, default_subcommand)
        }
    } else {
        determine_subcommand(&fixture.tags, default_subcommand)
    };

    // Build the CLI command using the resolved call config.
    let cmd_parts = build_cli_command(
        fixture,
        binary_name,
        &subcommand,
        static_cli_args,
        cli_flags,
        fixture.resolved_args(call_config),
    );

    if expects_error {
        let cmd = cmd_parts.join(" ");
        let body = render_error_test_body(&cmd, crate::e2e::codegen::declared_error_value(fixture));
        let _ = writeln!(out, "{body}");
        crate::e2e::codegen::error_path_assertions::emit(out, fixture, "  # ", "brew");
        let _ = writeln!(out, "}}");
        return;
    }

    // Check if any assertion will actually emit code (not be skipped).
    let has_active_assertions = fixture.assertions.iter().any(|a| {
        a.field
            .as_ref()
            .is_none_or(|f| f.is_empty() || field_resolver.is_valid_for_result(f))
    });

    // Capture output (only if there are active assertions that reference it).
    let cmd = cmd_parts.join(" ");
    if has_active_assertions {
        let _ = writeln!(out, "  local output");
        let _ = writeln!(out, "  output=$({cmd})");
    } else {
        let _ = writeln!(out, "  {cmd} >/dev/null");
    }
    let _ = writeln!(out);

    // Emit assertions. `out` accumulates every fixture's rendered function in this
    // category file (see `render_category_file`'s loop), so the strict-availability
    // scan below must only look at the text this fixture's own loop appended —
    // scanning the whole buffer would misattribute an earlier fixture's skip
    // comment to this fixture's id. ~keep
    let assertions_start = out.len();
    for assertion in &fixture.assertions {
        render_assertion(out, assertion, binary_name, field_resolver);
    }
    crate::e2e::codegen::fail_on_unavailable_field_markers(
        &out[assertions_start..],
        "brew",
        &fixture.id,
        &fixture.assertions,
    );
    crate::e2e::codegen::fail_on_unsupported_assertion_type_markers(&out[assertions_start..], "brew", &fixture.id);

    let _ = writeln!(out, "}}");
}

/// Determine the brew subcommand based on fixture tags.
///
/// If the fixture tags contain "crawl", returns "crawl".
/// If the fixture tags contain "map", returns "map".
/// Otherwise, returns the default subcommand.
pub(super) fn determine_subcommand(tags: &[String], default: &str) -> String {
    for tag in tags {
        if tag == "crawl" {
            return "crawl".to_string();
        }
        if tag == "map" {
            return "map".to_string();
        }
    }
    default.to_string()
}

/// Build the shell CLI invocation as a list of tokens.
///
/// Tokens are returned unquoted where safe (flag names) or single-quoted
/// (string values from the fixture).
pub(super) fn build_cli_command(
    fixture: &Fixture,
    binary_name: &str,
    subcommand: &str,
    static_cli_args: &[String],
    cli_flags: &std::collections::HashMap<String, String>,
    args: &[crate::e2e::config::ArgMapping],
) -> Vec<String> {
    let mut parts: Vec<String> = vec![binary_name.to_string(), subcommand.to_string()];

    for arg in args {
        match arg.arg_type.as_str() {
            "mock_url" => {
                if let Some(url) = crate::e2e::codegen::preserved_url_literal(
                    fixture.preserve_input_urls,
                    crate::e2e::codegen::resolve_field(&fixture.input, &arg.field),
                ) {
                    // ~keep Single quotes, not double. `escape_shell` neutralises `'` for a
                    // single-quoted context, and only single quotes suppress `$` and
                    // backtick expansion. Wrapping its output in double quotes — as the
                    // relative-path branch below must, to expand `${MOCK_SERVER_*}` — would
                    // let a fixture URL containing `$(...)` execute when the test runs.
                    parts.push(format!("'{}'", escape_shell(url)));
                    continue;
                }
                // Positional URL argument.
                //
                // Prefer the per-fixture `MOCK_SERVER_<FIXTURE_ID>` env var when set —
                // host-root fixtures (robots.txt, sitemap.xml) need their own listener
                // so the path lives at `/robots.txt`, not `/fixtures/<id>/robots.txt`.
                // Fall back to `MOCK_SERVER_URL/fixtures/<id>` for the common case.
                let upper_id = fixture.id.to_uppercase();
                parts.push(format!(
                    "\"${{MOCK_SERVER_{upper_id}:-${{MOCK_SERVER_URL}}/fixtures/{}}}\"",
                    fixture.id
                ));
            }
            "mock_url_list" => {
                // Multiple positional URL arguments.
                //
                // Resolve the list field from fixture input (with aliasing support),
                // then emit each URL as a positional argument.
                // Per-fixture env var for base URL (for host-root fixtures), falling back
                // to MOCK_SERVER_URL/fixtures/<fixture_id>.
                let upper_id = fixture.id.to_uppercase();
                let base_var = format!(
                    "${{MOCK_SERVER_{upper_id}:-${{MOCK_SERVER_URL}}/fixtures/{}}}",
                    fixture.id
                );
                let urls_value = crate::e2e::codegen::resolve_urls_field(&fixture.input, &arg.field);

                if let Some(urls_array) = urls_value.as_array() {
                    for url_value in urls_array {
                        if let Some(path) = url_value.as_str() {
                            if fixture.preserve_input_urls || path.starts_with("http") {
                                // Absolute URL — emit verbatim (shell-quoted).
                                parts.push(format!("\"{}\"", escape_shell(path)));
                            } else {
                                // Relative path — prepend base URL.
                                // Path typically includes leading `/`, e.g., `/page1`.
                                // Emit as a double-quoted token so shell expands the ${...} variable.
                                parts.push(format!("\"{}{}", base_var, path) + "\"");
                            }
                        }
                    }
                }
            }
            "handle" => {
                // CLI manages its own engine; skip handle args.
            }
            _ => {
                // Check if there is a cli_flags mapping for this field.
                if let Some(flag) = cli_flags.get(&arg.field) {
                    let field = arg.field.strip_prefix("input.").unwrap_or(&arg.field);
                    if let Some(val) = fixture.input.get(field)
                        && !val.is_null()
                    {
                        let val_str = json_value_to_shell_arg(val);
                        parts.push(flag.clone());
                        parts.push(val_str);
                    }
                }
            }
        }
    }

    // Check if fixture has input.config and emit it as --config flag.
    if let Some(config_val) = fixture.input.get("config")
        && !config_val.is_null()
    {
        // Minify the JSON config object to a single line for shell argument.
        let config_json = serde_json::to_string(config_val).unwrap_or_default();
        parts.push("--config".to_string());
        parts.push(format!("'{}'", escape_shell(&config_json)));
    }

    // Append static CLI args last.
    for static_arg in static_cli_args {
        parts.push(static_arg.clone());
    }

    parts
}

/// Convert a JSON value to a shell argument string.
///
/// Strings are wrapped in single quotes with embedded single quotes escaped.
/// Numbers and booleans are emitted verbatim.
fn json_value_to_shell_arg(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => format!("'{}'", escape_shell(s)),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => "''".to_string(),
        other => format!("'{}'", escape_shell(&other.to_string())),
    }
}

/// Convert a fixture field path to a jq expression.
///
/// A path like `metadata.title` becomes `.metadata.title`.
/// An array field like `links` becomes `.links`.
/// The pseudo-property `length` (also `count`, `size`) becomes `| length`
/// because jq uses pipe syntax for the `length` builtin.
fn field_to_jq_path(resolved: &str) -> String {
    // Check if the path ends with a length/count/size pseudo-property.
    // E.g., "pages.length" → ".pages | length"
    if let Some((prefix, suffix)) = resolved.rsplit_once('.')
        && (suffix == "length" || suffix == "count" || suffix == "size")
    {
        return format!(".{prefix} | length");
    }
    // Handle bare "length" / "count" / "size" (top-level array).
    if resolved == "length" || resolved == "count" || resolved == "size" {
        return ". | length".to_string();
    }
    format!(".{resolved}")
}

/// Build a CLI command for a method_result assertion.
///
/// Uses generic dispatch: `{binary_name} {kebab-method} "$output" args...`.
/// The method name is converted from snake_case to kebab-case for the CLI subcommand.
/// Args from the fixture JSON object are emitted as positional shell arguments in
/// insertion order, using best-effort shell quoting.
fn build_brew_method_call(binary_name: &str, method_name: &str, args: Option<&serde_json::Value>) -> String {
    let subcommand = method_name.replace('_', "-");
    if let Some(args_val) = args {
        let arg_str = args_val
            .as_object()
            .map(|obj| {
                obj.values()
                    .map(|v| match v {
                        serde_json::Value::String(s) => format!("'{}'", escape_shell(s)),
                        other => other.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        if arg_str.is_empty() {
            format!("{binary_name} {subcommand} \"$output\"")
        } else {
            format!("{binary_name} {subcommand} \"$output\" {arg_str}")
        }
    } else {
        format!("{binary_name} {subcommand} \"$output\"")
    }
}

fn render_check(out: &mut String, kind: &str, actual: &str, expected: &str) {
    let operator = match kind {
        "greater_than" => ">",
        "greater_than_or_equal" | "count_min" => ">=",
        "less_than" => "<",
        "less_than_or_equal" => "<=",
        _ => "==",
    };
    out.push_str(&crate::e2e::template_env::render(
        "brew/bats_assertion.jinja",
        crate::alef_context! { kind => kind, actual => actual, expected => expected, operator => operator,
        needs_literal_comment => expected.contains(['$', '`']), },
    ));
}

/// Render a single assertion as shell code.
fn render_assertion(out: &mut String, assertion: &Assertion, binary_name: &str, field_resolver: &FieldResolver) {
    // Skip assertions on fields not available on the result type.
    if let Some(f) = &assertion.field
        && !f.is_empty()
        && !field_resolver.is_valid_for_result(f)
    {
        let _ = writeln!(out, "  # skipped: {}", FieldSkip::NotAvailableOnResultType.message(f));
        return;
    }

    match assertion.assertion_type.as_str() {
        "equals" => value_capture::render_equals(out, assertion, field_resolver),
        "contains" => {
            if let Some(field) = &assertion.field
                && let Some(expected) = &assertion.value
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let expected_str = json_value_to_shell_string(expected);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                render_check(
                    out,
                    "contains",
                    &format!("val_{safe_field}"),
                    &format!("'{expected_str}'"),
                );
            }
        }
        "not_empty" | "tree_not_null" => {
            if let Some(field) = &assertion.field {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                render_check(out, "not_empty", &format!("val_{safe_field}"), "''");
            }
        }
        "count_min" | "root_child_count_min" => {
            if let Some(field) = &assertion.field
                && let Some(val) = &assertion.value
                && let Some(min) = val.as_u64()
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let safe_field = sanitize_ident(field);
                let _ = writeln!(out, "  local count_{safe_field}");
                let _ = writeln!(
                    out,
                    "  count_{safe_field}=$(echo \"$output\" | jq '{jq_path} | length')"
                );
                render_check(out, "count_min", &format!("count_{safe_field}"), &min.to_string());
            }
        }
        "greater_than" => {
            if let Some(field) = &assertion.field
                && let Some(val) = &assertion.value
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let threshold = json_value_to_shell_string(val);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                render_check(
                    out,
                    "greater_than",
                    &format!("val_{safe_field}"),
                    &format!("'{threshold}'"),
                );
            }
        }
        "greater_than_or_equal" => {
            if let Some(field) = &assertion.field
                && let Some(val) = &assertion.value
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let threshold = json_value_to_shell_string(val);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                render_check(
                    out,
                    "greater_than_or_equal",
                    &format!("val_{safe_field}"),
                    &format!("'{threshold}'"),
                );
            }
        }
        "contains_all" => {
            if let Some(field) = &assertion.field
                && let Some(serde_json::Value::Array(items)) = &assertion.value
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                for item in items {
                    let item_str = json_value_to_shell_string(item);
                    render_check(out, "contains", &format!("val_{safe_field}"), &format!("'{item_str}'"));
                }
            }
        }
        "is_empty" => {
            if let Some(field) = &assertion.field {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &format!("{jq_path} // empty"), field);
                render_check(out, "is_empty", &format!("val_{safe_field}"), "''");
            }
        }
        "less_than" => {
            if let Some(field) = &assertion.field
                && let Some(val) = &assertion.value
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let threshold = json_value_to_shell_string(val);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                render_check(
                    out,
                    "less_than",
                    &format!("val_{safe_field}"),
                    &format!("'{threshold}'"),
                );
            }
        }
        "not_contains" => {
            if let Some(field) = &assertion.field
                && let Some(expected) = &assertion.value
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let expected_str = json_value_to_shell_string(expected);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                render_check(
                    out,
                    "not_contains",
                    &format!("val_{safe_field}"),
                    &format!("'{expected_str}'"),
                );
            }
        }
        "count_equals" => {
            if let Some(field) = &assertion.field
                && let Some(val) = &assertion.value
                && let Some(n) = val.as_u64()
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let safe_field = sanitize_ident(field);
                let _ = writeln!(out, "  local count_{safe_field}");
                let _ = writeln!(
                    out,
                    "  count_{safe_field}=$(echo \"$output\" | jq '{jq_path} | length')"
                );
                let _ = writeln!(out, "  [ \"$count_{safe_field}\" -eq {n} ] || exit 1");
            }
        }
        "is_true" => {
            if let Some(field) = &assertion.field {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                let _ = writeln!(out, "  [ \"$val_{safe_field}\" = \"true\" ] || exit 1");
            }
        }
        "is_false" => {
            if let Some(field) = &assertion.field {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                let _ = writeln!(out, "  [ \"$val_{safe_field}\" = \"false\" ] || exit 1");
            }
        }
        "less_than_or_equal" => {
            if let Some(field) = &assertion.field
                && let Some(val) = &assertion.value
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let threshold = json_value_to_shell_string(val);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                render_check(
                    out,
                    "less_than_or_equal",
                    &format!("val_{safe_field}"),
                    &format!("'{threshold}'"),
                );
            }
        }
        "method_result" => {
            if let Some(method_name) = &assertion.method {
                let check = assertion.check.as_deref().unwrap_or("is_true");
                let cmd = build_brew_method_call(binary_name, method_name, assertion.args.as_ref());
                // For is_error, skip capturing the result — just run the command and check
                // the exit code so we don't execute the method twice.
                if check == "is_error" {
                    let _ = writeln!(out, "  if {cmd} >/dev/null 2>&1; then");
                    let _ = writeln!(
                        out,
                        "    echo 'FAIL [method_result]: expected method to raise error but it succeeded' >&2"
                    );
                    let _ = writeln!(out, "    return 1");
                    let _ = writeln!(out, "  fi");
                } else {
                    let method_var = format!("method_result_{}", sanitize_ident(method_name));
                    let _ = writeln!(out, "  local {method_var}");
                    let _ = writeln!(out, "  {method_var}=$({cmd})");
                    match check {
                        "equals" => {
                            if let Some(val) = &assertion.value {
                                let expected = json_value_to_shell_string(val);
                                let _ = writeln!(out, "  [ \"${method_var}\" = '{expected}' ] || exit 1");
                            }
                        }
                        "is_true" => {
                            let _ = writeln!(out, "  [ \"${method_var}\" = \"true\" ] || exit 1");
                        }
                        "is_false" => {
                            let _ = writeln!(out, "  [ \"${method_var}\" = \"false\" ] || exit 1");
                        }
                        "greater_than_or_equal" => {
                            if let Some(val) = &assertion.value
                                && let Some(n) = val.as_u64()
                            {
                                let _ = writeln!(out, "  [ \"${method_var}\" -ge {n} ] || exit 1");
                            }
                        }
                        "count_min" => {
                            if let Some(val) = &assertion.value
                                && let Some(n) = val.as_u64()
                            {
                                let _ = writeln!(
                                    out,
                                    "  local count_from_method_result=$(echo \"${method_var}\" | jq 'length')"
                                );
                                let _ = writeln!(out, "  [ \"$count_from_method_result\" -ge {n} ] || exit 1");
                            }
                        }
                        "contains" => {
                            if let Some(val) = &assertion.value {
                                let expected = json_value_to_shell_string(val);
                                let _ = writeln!(out, "  [[ \"${method_var}\" == *'{expected}'* ]] || exit 1");
                            }
                        }
                        other_check => {
                            panic!("Brew e2e generator: unsupported method_result check type: {other_check}");
                        }
                    }
                }
            } else {
                panic!("method_result assertion missing 'method' field");
            }
        }
        "min_length" => {
            if let Some(field) = &assertion.field
                && let Some(val) = &assertion.value
                && let Some(n) = val.as_u64()
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                let _ = writeln!(
                    out,
                    "  [ \"${{#val_{safe_field}}}\" -ge {n} ] || {{ echo \"FAIL [{field}]: expected length >= {n}\" >&2; return 1; }}"
                );
            }
        }
        "max_length" => {
            if let Some(field) = &assertion.field
                && let Some(val) = &assertion.value
                && let Some(n) = val.as_u64()
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                let _ = writeln!(
                    out,
                    "  [ \"${{#val_{safe_field}}}\" -le {n} ] || {{ echo \"FAIL [{field}]: expected length <= {n}\" >&2; return 1; }}"
                );
            }
        }
        "ends_with" => {
            if let Some(field) = &assertion.field
                && let Some(expected) = &assertion.value
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let expected_str = json_value_to_shell_string(expected);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                let _ = writeln!(
                    out,
                    "  [[ \"$val_{safe_field}\" == *'{expected_str}' ]] || {{ echo \"FAIL [{field}]: expected to end with '{expected_str}'\" >&2; return 1; }}"
                );
            }
        }
        "matches_regex" => {
            if let Some(field) = &assertion.field
                && let Some(expected) = &assertion.value
                && let Some(pattern) = expected.as_str()
            {
                let resolved = field_resolver.result_relative_path(field);
                let jq_path = field_to_jq_path(&resolved);
                let safe_field = sanitize_ident(field);
                value_capture::render(out, &safe_field, &jq_path, field);
                let _ = writeln!(
                    out,
                    "  [[ \"$val_{safe_field}\" =~ {pattern} ]] || {{ echo \"FAIL [{field}]: expected to match /{pattern}/\" >&2; return 1; }}"
                );
            }
        }
        "not_error" => {
            // No-op: reaching this point means the call succeeded.
        }
        "error" => {
            // Handled at the function level (early return above).
        }
        other => {
            panic!("Brew e2e generator: unsupported assertion type: {other}");
        }
    }
}

/// Convert a JSON value to a plain string suitable for use in shell assertions.
///
/// Returns the bare string content (no quotes) — callers wrap in single quotes.
fn json_value_to_shell_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => escape_shell(s),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => String::new(),
        other => escape_shell(&other.to_string()),
    }
}

#[cfg(test)]
mod render_error_test_body_tests;

#[cfg(test)]
mod unavailable_field_marker_tests;

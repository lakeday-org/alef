use super::{Fixture, is_host_root_path, origin_tokens};
use serde_json::Value;

impl Fixture {
    /// Whether generated URLs must consult the runtime's dedicated fixture origin.
    ///
    /// File bodies are opaque during generation: the runtime resolves their HTML and
    /// origin tokens before deciding whether to allocate a dedicated listener. Consult
    /// that listener when present and retain each language's shared URL fallback. ~keep
    pub fn has_host_root_route(&self) -> bool {
        self.input
            .get("mock_responses")
            .and_then(Value::as_array)
            .is_some_and(|routes| routes.iter().any(route_may_require_origin))
    }
}

fn route_may_require_origin(entry: &Value) -> bool {
    entry.get("path").and_then(Value::as_str).is_some_and(is_host_root_path)
        || entry
            .get("body_file")
            .and_then(Value::as_str)
            .is_some_and(|path| !path.is_empty())
        || headers_require_origin(entry)
        || entry
            .get("body_inline")
            .and_then(Value::as_str)
            .is_some_and(body_requires_origin)
        || origin_tokens::entry_has_origin_token(entry)
}

fn headers_require_origin(entry: &Value) -> bool {
    let status = entry.get("status_code").and_then(Value::as_u64).unwrap_or(0);
    entry.get("headers").and_then(Value::as_object).is_some_and(|headers| {
        headers.iter().any(|(name, value)| {
            value.as_str().is_some_and(|value| {
                (name.eq_ignore_ascii_case("location") && (300..400).contains(&status) && value.starts_with('/'))
                    || (name.eq_ignore_ascii_case("refresh") && refresh_targets_origin(value))
            })
        })
    })
}

fn refresh_targets_origin(value: &str) -> bool {
    value
        .to_ascii_lowercase()
        .find("url=")
        .is_some_and(|index| value[index + 4..].trim_start().starts_with('/'))
}

fn body_requires_origin(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    let meta_refresh = lower
        .split("http-equiv=\"refresh\"")
        .nth(1)
        .and_then(|tail| tail.split("content=").nth(1))
        .is_some_and(|content| content.trim_start_matches(['"', '\'']).contains("url=/"));
    meta_refresh || body.contains("href=\"/") || body.contains("href='/")
}

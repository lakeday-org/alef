pub(crate) fn render(value: &str, extension: bool) -> String {
    let mut escaped = value.escape_default().to_string();
    // Template keep-marker stripping must not alter a caller's literal default. ~keep
    if extension {
        escaped = escaped.replace('~', "\\u{7e}");
    }
    format!("\"{escaped}\".to_string()")
}

/// Returns the URL from the first CLI argument, or `default` when absent.
pub fn initial_url_from_args(default: &str) -> String {
    std::env::args().nth(1).unwrap_or_else(|| default.to_string())
}

/// Prepends `https://` when the input has no scheme.
pub fn normalize_url(raw: &str) -> String {
    if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    }
}

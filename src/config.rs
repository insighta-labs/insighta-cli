/// Returns the backend API base URL.
///
/// Reads `INSIGHTA_API_URL` from the environment.
/// Falls back to `http://localhost:8000` if the variable is not set.
pub fn backend_url() -> String {
    std::env::var("INSIGHTA_API_URL").unwrap_or_else(|_| "http://localhost:8000".to_string())
}

/// Returns the local callback server port used during the OAuth flow.
///
/// Reads `INSIGHTA_CALLBACK_PORT` from the environment.
/// Falls back to `8182` if the variable is absent or unparseable.
pub fn callback_port() -> u16 {
    std::env::var("INSIGHTA_CALLBACK_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8182)
}

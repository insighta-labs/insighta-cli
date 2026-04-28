use std::env::var;

/// The backend API base URL.
pub fn backend_url() -> String {
    var("INSIGHTA_API_URL").unwrap_or_else(|_| "http://localhost:8000".to_string())
}

/// The local callback server port.
pub fn callback_port() -> u16 {
    var("INSIGHTA_CALLBACK_PORT")
        .ok()
        .and_then(|val| val.parse().ok())
        .unwrap_or(8182)
}

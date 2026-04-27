use std::fmt;

/// Represents all possible errors that can occur in the CLI application.
///
/// These errors are displayed to the user with human-readable messages
/// and determine the appropriate exit behavior.
#[derive(Debug)]
pub enum CliError {
    /// The user has not authenticated. Prompts them to run `insighta login`.
    NotLoggedIn,
    /// The session has expired. Prompts re-authentication.
    TokenExpired,
    /// The API returned an error response.
    Api(String),
    /// An IO-related failure (e.g., reading or writing credential files).
    Io(std::io::Error),
    /// An HTTP request failed (e.g., network unreachable or connection refused).
    Http(reqwest::Error),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::NotLoggedIn => write!(f, "Not logged in. Run `insighta login` first."),
            CliError::TokenExpired => {
                write!(
                    f,
                    "Session expired. Run `insighta login` to re-authenticate."
                )
            }
            CliError::Api(msg) => write!(f, "API error: {msg}"),
            CliError::Io(e) => write!(f, "IO error: {e}"),
            CliError::Http(e) => write!(f, "HTTP error: {e}"),
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Io(e)
    }
}

impl From<reqwest::Error> for CliError {
    fn from(e: reqwest::Error) -> Self {
        CliError::Http(e)
    }
}

pub type Result<T> = std::result::Result<T, CliError>;

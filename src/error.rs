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
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::NotLoggedIn => {
                write!(formatter, "Not logged in. Run `insighta login` first.")
            }
            CliError::TokenExpired => {
                write!(
                    formatter,
                    "Session expired. Run `insighta login` to re-authenticate."
                )
            }
            CliError::Api(msg) => write!(formatter, "API error: {msg}"),
            CliError::Io(err) => write!(formatter, "IO error: {err}"),
            CliError::Http(err) => write!(formatter, "HTTP error: {err}"),
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        CliError::Io(err)
    }
}

impl From<reqwest::Error> for CliError {
    fn from(err: reqwest::Error) -> Self {
        CliError::Http(err)
    }
}

pub type Result<T> = std::result::Result<T, CliError>;

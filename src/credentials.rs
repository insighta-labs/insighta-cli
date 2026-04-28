use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{CliError, Result};

/// Stores the authenticated user's session credentials, persisted locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub username: String,
}

fn credentials_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        CliError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not locate home directory",
        ))
    })?;
    Ok(home.join(".insighta").join("credentials.json"))
}

/// Loads stored credentials from the local credentials file.
///
/// # Returns
///
/// Returns a `Result` containing the deserialized `Credentials` on success.
///
/// # Errors
///
/// Returns `CliError::Io` if the home directory cannot be resolved.
/// Returns `CliError::NotLoggedIn` if the credentials file does not exist or cannot be parsed.
pub fn load() -> Result<Credentials> {
    let path = credentials_path()?;
    let raw = std::fs::read_to_string(&path).map_err(|_| CliError::NotLoggedIn)?;
    serde_json::from_str(&raw).map_err(|_| CliError::NotLoggedIn)
}

/// Saves credentials to the local credentials file, creating parent directories as needed.
///
/// # Arguments
///
/// * `creds` - A reference to the `Credentials` to persist.
///
/// # Returns
///
/// Returns `Ok(())` on success.
///
/// # Errors
///
/// Returns `CliError::Io` if the home directory cannot be resolved, directory creation fails,
/// serialization fails, or the file cannot be written.
pub fn save(creds: &Credentials) -> Result<()> {
    let path = credentials_path()?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let raw = serde_json::to_string_pretty(creds)
        .map_err(|err| CliError::Io(std::io::Error::other(err.to_string())))?;

    std::fs::write(&path, raw)?;
    Ok(())
}

/// Deletes the local credentials file, effectively logging the user out.
///
/// Does nothing if the credentials file does not already exist.
///
/// # Returns
///
/// Returns `Ok(())` on success.
///
/// # Errors
///
/// Returns `CliError::Io` if the home directory cannot be resolved or if file removal fails.
pub fn delete() -> Result<()> {
    let path = credentials_path()?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

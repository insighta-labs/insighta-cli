use std::time::Duration;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{
    config,
    credentials::{self, Credentials},
    error::{CliError, Result},
    output,
};

const CALLBACK_TIMEOUT_SECS: u64 = 180;

fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn derive_code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn extract_username_from_token(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    json["username"].as_str().map(|s| s.to_string())
}

async fn wait_for_callback(listener: tokio::net::TcpListener) -> Result<(String, String)> {
    let (stream, _) = listener.accept().await.map_err(CliError::Io)?;
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .map_err(CliError::Io)?;

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
            <html><body><p>Authentication complete. You can close this tab.</p></body></html>";
    write_half
        .write_all(response.as_bytes())
        .await
        .map_err(CliError::Io)?;

    // Request line: GET /callback?code=xxx&state=yyy HTTP/1.1
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_string();

    let query = path.split_once('?').map(|(_, q)| q).unwrap_or_default();
    let mut code = None;
    let mut state = None;
    let mut error = None;
    let mut error_description = None;

    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        match (parts.next(), parts.next()) {
            (Some("code"), Some(v)) => code = Some(v.to_string()),
            (Some("state"), Some(v)) => state = Some(v.to_string()),
            (Some("error"), Some(v)) => {
                error = Some(urlencoding::decode(v).unwrap_or_default().into_owned());
            }
            (Some("error_description"), Some(v)) => {
                error_description = Some(urlencoding::decode(v).unwrap_or_default().into_owned());
            }
            _ => {}
        }
    }

    if let Some(err) = error {
        let msg = error_description.unwrap_or(err);
        return Err(CliError::Api(format!(
            "GitHub authorization failed: {}",
            msg
        )));
    }

    match (code, state) {
        (Some(c), Some(s)) => Ok((c, s)),
        _ => Err(CliError::Api(
            "Callback did not contain expected parameters".to_string(),
        )),
    }
}

pub async fn login() -> Result<()> {
    let port = config::callback_port();
    let backend = config::backend_url();

    let verifier = generate_code_verifier();
    let challenge = derive_code_challenge(&verifier);
    let state = generate_state();

    let redirect_uri = format!("http://127.0.0.1:{}/callback", port);
    let encoded_redirect = urlencoding::encode(&redirect_uri);

    let auth_url = format!(
        "{}/auth/github?state={}&code_challenge={}&redirect_uri={}",
        backend, state, challenge, encoded_redirect
    );

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .map_err(|e| {
            CliError::Io(std::io::Error::new(
                e.kind(),
                format!("Could not bind to port {}: {}", port, e),
            ))
        })?;

    println!("Opening GitHub in your browser...");
    open::that(&auth_url).map_err(|e| {
        CliError::Io(std::io::Error::other(format!(
            "Could not open browser: {}",
            e
        )))
    })?;

    let pb = output::spinner("Waiting for GitHub authorization");
    let callback = tokio::time::timeout(
        Duration::from_secs(CALLBACK_TIMEOUT_SECS),
        wait_for_callback(listener),
    )
    .await
    .map_err(|_| {
        CliError::Api("Authorization timed out. Please run `insighta login` again.".to_string())
    })?;

    let (code, returned_state) = callback?;
    pb.finish_and_clear();

    if returned_state != state {
        return Err(CliError::Api("State mismatch — possible CSRF".to_string()));
    }

    let pb = output::spinner("Completing login");

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/auth/github/callback", backend))
        .query(&[
            ("code", code.as_str()),
            ("state", returned_state.as_str()),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .await?
        .json::<serde_json::Value>()
        .await
        .map_err(|_| CliError::Api("Failed to parse login response".to_string()))?;

    pb.finish_and_clear();

    let status = res["status"].as_str().unwrap_or("error");
    if status != "success" {
        let msg = res["message"]
            .as_str()
            .unwrap_or("Login failed")
            .to_string();
        return Err(CliError::Api(msg));
    }

    let access_token = res["access_token"]
        .as_str()
        .ok_or_else(|| CliError::Api("Missing access_token in response".to_string()))?
        .to_string();

    let refresh_token = res["refresh_token"]
        .as_str()
        .ok_or_else(|| CliError::Api("Missing refresh_token in response".to_string()))?
        .to_string();

    let username = extract_username_from_token(&access_token)
        .ok_or_else(|| CliError::Api("Could not read username from token".to_string()))?;

    credentials::save(&Credentials {
        access_token,
        refresh_token,
        username: username.clone(),
    })?;

    output::print_success(&format!("Logged in as @{}", username));
    Ok(())
}

pub async fn logout() -> Result<()> {
    let creds = credentials::load()?;

    let client = reqwest::Client::new();
    let server_result = client
        .post(format!("{}/auth/logout", config::backend_url()))
        .json(&serde_json::json!({ "refresh_token": creds.refresh_token }))
        .send()
        .await;

    // Always clear local credentials — the user wants to be logged out.
    credentials::delete()?;

    match server_result {
        Err(_) => {
            output::print_success(
                "Logged out. (Warning: could not reach server to invalidate session)",
            );
        }
        Ok(resp) => {
            let status = resp.status().as_u16();
            if status < 400 || status == 401 || status == 404 {
                // Success, or token was already invalid — both are fine.
                output::print_success("Logged out.");
            } else {
                output::print_success(&format!(
                    "Logged out. (Warning: server responded with {})",
                    status
                ));
            }
        }
    }

    Ok(())
}

pub async fn whoami() -> Result<()> {
    let pb = output::spinner("Verifying session");
    let res = crate::client::api_get("/auth/me", &[]).await;
    pb.finish_and_clear();

    let res = res?;
    let username = res["data"]["username"].as_str().unwrap_or("unknown");
    let role = res["data"]["role"].as_str().unwrap_or("analyst");
    output::print_success(&format!("Logged in as @{} ({})", username, role));
    Ok(())
}

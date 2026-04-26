use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
};

use base64::{Engine, engine::general_purpose::STANDARD, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use sha2::{Digest, Sha256};

use crate::{
    config,
    credentials::{self, Credentials},
    error::{CliError, Result},
    output,
};

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

    let padded = match payload.len() % 4 {
        2 => format!("{}==", payload),
        3 => format!("{}=", payload),
        _ => payload.to_string(),
    };

    let decoded = STANDARD.decode(&padded).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    json["username"].as_str().map(|s| s.to_string())
}

fn wait_for_callback(port: u16) -> Result<(String, String)> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).map_err(|e| {
        CliError::Io(std::io::Error::new(
            e.kind(),
            format!("Could not bind to port {}: {}", port, e),
        ))
    })?;

    let (mut stream, _) = listener.accept()?;
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Request line: GET /callback?code=xxx&state=yyy HTTP/1.1
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_string();

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
        <html><body><p>Authentication complete. You can close this tab.</p></body></html>";
    stream.write_all(response.as_bytes())?;

    let query = path.split_once('?').map(|x| x.1).unwrap_or_default();
    let mut code = None;
    let mut state = None;

    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        match (parts.next(), parts.next()) {
            (Some("code"), Some(v)) => code = Some(v.to_string()),
            (Some("state"), Some(v)) => state = Some(v.to_string()),
            _ => {}
        }
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

    let redirect_uri = format!("http://localhost:{}/callback", port);

    let auth_url = format!(
        "{}/auth/github?state={}&code_challenge={}&redirect_uri={}",
        backend, state, challenge, redirect_uri
    );

    println!("Opening GitHub in your browser...");
    open::that(&auth_url).map_err(|e| {
        CliError::Io(std::io::Error::other(format!(
            "Could not open browser: {}",
            e
        )))
    })?;

    let pb = output::spinner("Waiting for GitHub authorization");
    let (code, returned_state) = wait_for_callback(port)?;
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
    client
        .post(format!("{}/auth/logout", config::backend_url()))
        .json(&serde_json::json!({ "refresh_token": creds.refresh_token }))
        .send()
        .await
        .map_err(|e| CliError::Api(format!("Logout request failed: {}", e)))?;

    credentials::delete()?;
    output::print_success("Logged out.");
    Ok(())
}

pub fn whoami() -> Result<()> {
    let creds = credentials::load()?;
    output::print_success(&format!("Logged in as @{}", creds.username));
    Ok(())
}

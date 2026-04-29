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
    json["username"]
        .as_str()
        .map(|username| username.to_string())
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
<!DOCTYPE html>\n\
<html>\n\
<head>\n\
    <title>Insighta Labs - Authentication</title>\n\
    <style>\n\
        @import url('https://fonts.googleapis.com/css2?family=Anton&family=Space+Mono:wght@400;700&display=swap');\n\
        body { background: #2a1b38; color: #fdf6e3; font-family: 'Space Mono', 'Courier New', monospace; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; text-align: center; }\n\
        .card { background: #422d5b; border: none; border-radius: 8px; padding: 52px; box-shadow: 6px 6px 0px #000; max-width: 520px; width: calc(100% - 40px); }\n\
        @media (max-width: 480px) { .card { padding: 32px 20px; } }\n\
        .title { font-size: 36px; font-weight: 900; font-family: 'Anton', 'Impact', 'Arial Black', sans-serif; letter-spacing: 2px; color: #ff7b00; margin-bottom: 10px; }\n\
        .subtitle { color: #fdf6e3; font-size: 17px; font-weight: bold; letter-spacing: 1px; margin-bottom: 42px; text-transform: uppercase; border-bottom: 3px solid #000; padding-bottom: 16px; display: inline-block; }\n\
        .message { color: #fdf6e3; font-size: 18px; line-height: 1.6; font-weight: bold; }\n\
        svg { margin-bottom: 31px; stroke: #ff7b00; }\n\
        .btn { margin-top: 32px; background: transparent; border: 3px solid #ff7b00; color: #ff7b00; padding: 12px 24px; font-family: 'Space Mono', monospace; font-weight: bold; cursor: pointer; text-transform: uppercase; letter-spacing: 2px; border-radius: 4px; }\n\
    </style>\n\
</head>\n\
<body>\n\
    <div class=\"card\">\n\
        <svg width=\"62\" height=\"62\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"3\" stroke-linecap=\"square\" stroke-linejoin=\"miter\">\n\
            <path d=\"M22 11.08V12a10 10 0 1 1-5.93-9.14\"></path>\n\
            <polyline points=\"22 4 12 14.01 9 11.01\"></polyline>\n\
        </svg>\n\
        <div class=\"title\">INSIGHTA LABS+</div>\n\
        <div class=\"subtitle\">Intelligence Platform</div>\n\
        <div class=\"message\">Authentication complete.<br><br>You can now return to the CLI. This tab will close automatically in 3 seconds.</div>\n\
        <button onclick=\"window.close()\" class=\"btn\">Close Tab</button>\n\
    </div>\n\
    <script>setTimeout(() => window.close(), 3000);</script>\n\
</body>\n\
</html>";
    write_half
        .write_all(response.as_bytes())
        .await
        .map_err(CliError::Io)?;

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
        return Err(CliError::Api(format!("GitHub authorization failed: {msg}")));
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

    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let encoded_redirect = urlencoding::encode(&redirect_uri);

    let auth_url = format!(
        "{backend}/auth/github?state={state}&code_challenge={challenge}&redirect_uri={encoded_redirect}"
    );

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .map_err(|err| {
            CliError::Io(std::io::Error::new(
                err.kind(),
                format!("Could not bind to port {port}: {err}"),
            ))
        })?;

    println!("Opening GitHub in your browser...");
    open::that(&auth_url).map_err(|err| {
        CliError::Io(std::io::Error::other(format!(
            "Could not open browser: {err}"
        )))
    })?;

    let spinner = output::spinner("Waiting for GitHub authorization");
    let callback = tokio::time::timeout(
        Duration::from_secs(CALLBACK_TIMEOUT_SECS),
        wait_for_callback(listener),
    )
    .await
    .map_err(|_| {
        CliError::Api("Authorization timed out. Please run `insighta login` again.".to_string())
    })?;

    let (code, returned_state) = callback?;
    spinner.finish_and_clear();

    if returned_state != state {
        return Err(CliError::Api("State mismatch — possible CSRF".to_string()));
    }

    let spinner = output::spinner("Completing login");

    let client = reqwest::Client::new();
    let token_response = client
        .get(format!("{backend}/auth/github/callback"))
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

    spinner.finish_and_clear();

    let login_status = token_response["status"].as_str().unwrap_or("error");
    if login_status != "success" {
        let msg = token_response["message"]
            .as_str()
            .unwrap_or("Login failed")
            .to_string();
        return Err(CliError::Api(msg));
    }

    let access_token = token_response["access_token"]
        .as_str()
        .ok_or_else(|| CliError::Api("Missing access_token in response".to_string()))?
        .to_string();

    let refresh_token = token_response["refresh_token"]
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

    output::print_success(&format!("Logged in as @{username}"));
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
                    "Logged out. (Warning: server responded with {status})"
                ));
            }
        }
    }

    Ok(())
}

pub async fn whoami() -> Result<()> {
    let spinner = output::spinner("Verifying session");
    let me_response = crate::client::api_get("/auth/me", &[]).await;
    spinner.finish_and_clear();

    let me_response = me_response?;
    let username = me_response["data"]["username"]
        .as_str()
        .unwrap_or("unknown");
    let role = me_response["data"]["role"].as_str().unwrap_or("analyst");
    output::print_success(&format!("Logged in as @{username} ({role})"));
    Ok(())
}

use reqwest::{Method, Response};
use serde_json::Value;
use std::sync::OnceLock;

use crate::{
    config,
    credentials::{self, Credentials},
    error::{CliError, Result},
};

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_client() -> &'static reqwest::Client {
    CLIENT.get_or_init(reqwest::Client::new)
}

#[derive(serde::Deserialize)]
struct RefreshResponse {
    status: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

async fn refresh_credentials(credentials: &Credentials) -> Result<Credentials> {
    let client = get_client();
    let res = client
        .post(format!("{}/auth/refresh", config::backend_url()))
        .json(&serde_json::json!({ "refresh_token": credentials.refresh_token }))
        .send()
        .await?
        .json::<RefreshResponse>()
        .await
        .map_err(|_| CliError::TokenExpired)?;

    if res.status != "success" {
        credentials::delete().ok();
        return Err(CliError::TokenExpired);
    }

    let new_creds = Credentials {
        access_token: res.access_token.ok_or(CliError::TokenExpired)?,
        refresh_token: res.refresh_token.ok_or(CliError::TokenExpired)?,
        username: credentials.username.clone(),
    };

    credentials::save(&new_creds)?;
    Ok(new_creds)
}

/// Base function to perform an authenticated request with automatic 401 refresh/retry.
async fn authenticated_request(
    method: Method,
    path: &str,
    query: &[(&str, &str)],
    body: &Option<Value>,
) -> Result<Response> {
    let mut credentials = credentials::load()?;
    let client = get_client();

    let response = send_request(client, &method, path, query, body, &credentials).await?;

    if response.status().as_u16() == 401 {
        credentials = refresh_credentials(&credentials).await?;
        return send_request(client, &method, path, query, body, &credentials).await;
    }

    Ok(response)
}

pub async fn request(
    method: Method,
    path: &str,
    query: &[(&str, &str)],
    body: Option<Value>,
) -> Result<Value> {
    let response = authenticated_request(method, path, query, &body).await?;
    parse_response(response).await
}

/// Sends an authenticated GET and returns the raw response without parsing JSON.
pub async fn raw_get(path: &str, query: &[(&str, &str)]) -> Result<Response> {
    authenticated_request(Method::GET, path, query, &None).await
}

async fn send_request(
    client: &reqwest::Client,
    method: &Method,
    path: &str,
    query: &[(&str, &str)],
    body: &Option<Value>,
    credentials: &Credentials,
) -> Result<Response> {
    let url = format!("{}{}", config::backend_url(), path);
    let mut request_builder = client
        .request(method.clone(), &url)
        .header(
            "Authorization",
            format!("Bearer {}", credentials.access_token),
        )
        .header("X-API-Version", "1")
        .query(query);

    if let Some(json_body) = body {
        request_builder = request_builder.json(json_body);
    }

    request_builder.send().await.map_err(CliError::Http)
}

async fn parse_response(response: Response) -> Result<Value> {
    let status = response.status().as_u16();
    let json: Value = response
        .json()
        .await
        .map_err(|_| CliError::Api("Failed to parse response".to_string()))?;

    if status >= 400 {
        let msg = json["message"]
            .as_str()
            .unwrap_or("Unknown error")
            .to_string();
        return Err(CliError::Api(msg));
    }

    Ok(json)
}

pub async fn api_get(path: &str, query: &[(&str, &str)]) -> Result<Value> {
    request(Method::GET, path, query, None).await
}

pub async fn api_post(path: &str, body: Value) -> Result<Value> {
    request(Method::POST, path, &[], Some(body)).await
}

use reqwest::{Method, Response};
use serde_json::Value;

use crate::{
    credentials::{self, Credentials},
    error::{CliError, Result},
};

const BACKEND_URL: &str = "http://localhost:8000";

#[derive(serde::Deserialize)]
struct RefreshResponse {
    status: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

async fn refresh_credentials(creds: &Credentials) -> Result<Credentials> {
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/auth/refresh", BACKEND_URL))
        .json(&serde_json::json!({ "refresh_token": creds.refresh_token }))
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
        username: creds.username.clone(),
    };

    credentials::save(&new_creds)?;
    Ok(new_creds)
}

pub async fn request(
    method: Method,
    path: &str,
    query: &[(&str, &str)],
    body: Option<Value>,
) -> Result<Value> {
    let mut creds = credentials::load()?;
    let client = reqwest::Client::new();

    let response = send_request(&client, &method, path, query, &body, &creds).await?;

    if response.status().as_u16() == 401 {
        creds = refresh_credentials(&creds).await?;
        let retried = send_request(&client, &method, path, query, &body, &creds).await?;
        return parse_response(retried).await;
    }

    parse_response(response).await
}

async fn send_request(
    client: &reqwest::Client,
    method: &Method,
    path: &str,
    query: &[(&str, &str)],
    body: &Option<Value>,
    creds: &Credentials,
) -> Result<Response> {
    let url = format!("{}{}", BACKEND_URL, path);
    let mut req = client
        .request(method.clone(), &url)
        .header("Authorization", format!("Bearer {}", creds.access_token))
        .header("X-API-Version", "1")
        .query(query);

    if let Some(b) = body {
        req = req.json(b);
    }

    req.send().await.map_err(CliError::Http)
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

pub async fn api_delete(path: &str) -> Result<Value> {
    request(Method::DELETE, path, &[], None).await
}

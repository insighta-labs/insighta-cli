use serde_json::Value;

use crate::{
    cli::ProfileCommands,
    client::{api_get, api_post, raw_get},
    error::{CliError, Result},
    output,
};

struct ListOptions {
    gender: Option<String>,
    country: Option<String>,
    age_group: Option<String>,
    min_age: Option<u8>,
    max_age: Option<u8>,
    sort_by: Option<String>,
    order: Option<String>,
    page: u32,
    limit: u32,
}

pub async fn handle(cmd: ProfileCommands) -> Result<()> {
    match cmd {
        ProfileCommands::List {
            gender,
            country,
            age_group,
            min_age,
            max_age,
            sort_by,
            order,
            page,
            limit,
        } => {
            list(ListOptions {
                gender,
                country,
                age_group,
                min_age,
                max_age,
                sort_by,
                order,
                page,
                limit,
            })
            .await
        }

        ProfileCommands::Get { id } => get(&id).await,

        ProfileCommands::Search { query, page, limit } => search(&query, page, limit).await,

        ProfileCommands::Create { name } => create(&name).await,

        ProfileCommands::Export {
            format,
            gender,
            country,
        } => export(&format, gender, country).await,
    }
}

async fn list(options: ListOptions) -> Result<()> {
    let pb = output::spinner("Fetching profiles");

    let page_s = options.page.to_string();
    let limit_s = options.limit.to_string();
    let min_age_s = options.min_age.map(|v| v.to_string());
    let max_age_s = options.max_age.map(|v| v.to_string());

    let mut query: Vec<(&str, &str)> = vec![("page", &page_s), ("limit", &limit_s)];

    if let Some(ref g) = options.gender {
        query.push(("gender", g));
    }
    if let Some(ref c) = options.country {
        query.push(("country_id", c));
    }
    if let Some(ref a) = options.age_group {
        query.push(("age_group", a));
    }
    if let Some(ref m) = min_age_s {
        query.push(("min_age", m));
    }
    if let Some(ref m) = max_age_s {
        query.push(("max_age", m));
    }
    if let Some(ref s) = options.sort_by {
        query.push(("sort_by", s));
    }
    if let Some(ref o) = options.order {
        query.push(("order", o));
    }

    let res = api_get("/api/profiles", &query).await;
    pb.finish_and_clear();

    let res = res?;
    let profiles = res["data"].as_array().cloned().unwrap_or_default();

    if profiles.is_empty() {
        output::print_success("No profiles found.");
        return Ok(());
    }

    let total = res["total"].as_u64().unwrap_or(0);
    let total_pages = res["total_pages"].as_u64().unwrap_or(1);
    println!(
        "Showing page {} of {} ({} total)\n",
        options.page, total_pages, total
    );

    print_profile_table(&profiles);
    Ok(())
}

async fn get(id: &str) -> Result<()> {
    let pb = output::spinner("Fetching profile");
    let res = api_get(&format!("/api/profiles/{}", id), &[]).await;
    pb.finish_and_clear();

    let res = res?;
    let p = &res["data"];

    output::print_table(
        vec!["Field", "Value"],
        vec![
            vec!["ID".into(), str_val(p, "id")],
            vec!["Name".into(), str_val(p, "name")],
            vec!["Gender".into(), str_val(p, "gender")],
            vec![
                "Gender Probability".into(),
                str_val(p, "gender_probability"),
            ],
            vec!["Age".into(), str_val(p, "age")],
            vec!["Age Group".into(), str_val(p, "age_group")],
            vec!["Country".into(), str_val(p, "country_name")],
            vec!["Country Code".into(), str_val(p, "country_id")],
            vec![
                "Country Probability".into(),
                str_val(p, "country_probability"),
            ],
            vec!["Created At".into(), str_val(p, "created_at")],
        ],
    );

    Ok(())
}

async fn search(query: &str, page: u32, limit: u32) -> Result<()> {
    let pb = output::spinner("Searching profiles");
    let page_s = page.to_string();
    let limit_s = limit.to_string();
    let res = api_get(
        "/api/profiles/search",
        &[("q", query), ("page", &page_s), ("limit", &limit_s)],
    )
    .await;
    pb.finish_and_clear();

    let res = res?;
    let profiles = res["data"].as_array().cloned().unwrap_or_default();

    if profiles.is_empty() {
        output::print_success("No profiles found.");
        return Ok(());
    }

    let total = res["total"].as_u64().unwrap_or(0);
    let total_pages = res["total_pages"].as_u64().unwrap_or(1);
    println!(
        "Showing page {} of {} ({} total)\n",
        page, total_pages, total
    );

    print_profile_table(&profiles);
    Ok(())
}

async fn create(name: &str) -> Result<()> {
    let pb = output::spinner(&format!("Creating profile for '{}'", name));
    let res = api_post("/api/profiles", serde_json::json!({ "name": name })).await;
    pb.finish_and_clear();

    let res = res?;
    let p = &res["data"];

    output::print_success("Profile created.");
    output::print_table(
        vec!["Field", "Value"],
        vec![
            vec!["ID".into(), str_val(p, "id")],
            vec!["Name".into(), str_val(p, "name")],
            vec!["Gender".into(), str_val(p, "gender")],
            vec!["Age".into(), str_val(p, "age")],
            vec!["Country".into(), str_val(p, "country_name")],
        ],
    );

    Ok(())
}

async fn export(format: &str, gender: Option<String>, country: Option<String>) -> Result<()> {
    let pb = output::spinner("Exporting profiles");

    let mut query: Vec<(&str, &str)> = vec![("format", format)];
    if let Some(ref g) = gender {
        query.push(("gender", g));
    }
    if let Some(ref c) = country {
        query.push(("country_id", c));
    }

    // raw_get handles auth and token refresh while returning the binary body directly.
    let response = raw_get("/api/profiles/export", &query).await;
    pb.finish_and_clear();

    let response = response?;

    if !response.status().is_success() {
        let json: serde_json::Value = response.json().await.unwrap_or_default();
        let msg = json["message"]
            .as_str()
            .unwrap_or("Export failed")
            .to_string();
        return Err(CliError::Api(msg));
    }

    let filename = response
        .headers()
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split("filename=").nth(1))
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_else(|| "profiles_export.csv".to_string());

    let bytes = response.bytes().await?;
    std::fs::write(&filename, &bytes)?;

    output::print_success(&format!("Exported to {}", filename));
    Ok(())
}

fn print_profile_table(profiles: &[Value]) {
    output::print_table(
        vec![
            "ID",
            "Name",
            "Gender",
            "Age",
            "Age Group",
            "Country",
            "Created At",
        ],
        profiles
            .iter()
            .map(|p| {
                vec![
                    str_val(p, "id"),
                    str_val(p, "name"),
                    str_val(p, "gender"),
                    str_val(p, "age"),
                    str_val(p, "age_group"),
                    str_val(p, "country_name"),
                    str_val(p, "created_at"),
                ]
            })
            .collect(),
    );
}

fn str_val(v: &Value, key: &str) -> String {
    match &v[key] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}

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
            age_group,
            min_age,
            max_age,
            sort_by,
            order,
        } => {
            export(
                &format, gender, country, age_group, min_age, max_age, sort_by, order,
            )
            .await
        }
    }
}

async fn list(options: ListOptions) -> Result<()> {
    let spinner = output::spinner("Fetching profiles");

    let page_s = options.page.to_string();
    let limit_s = options.limit.to_string();
    let min_age_s = options.min_age.map(|val| val.to_string());
    let max_age_s = options.max_age.map(|val| val.to_string());

    let mut query: Vec<(&str, &str)> = vec![("page", &page_s), ("limit", &limit_s)];

    if let Some(ref gender) = options.gender {
        query.push(("gender", gender));
    }
    if let Some(ref country) = options.country {
        query.push(("country_id", country));
    }
    if let Some(ref age_group) = options.age_group {
        query.push(("age_group", age_group));
    }
    if let Some(ref age) = min_age_s {
        query.push(("min_age", age));
    }
    if let Some(ref age) = max_age_s {
        query.push(("max_age", age));
    }
    if let Some(ref sort) = options.sort_by {
        query.push(("sort_by", sort));
    }
    if let Some(ref order) = options.order {
        query.push(("order", order));
    }

    let api_response = api_get("/api/profiles", &query).await;
    spinner.finish_and_clear();

    let api_response = api_response?;
    let profiles = api_response["data"].as_array().cloned().unwrap_or_default();

    if profiles.is_empty() {
        output::print_success("No profiles found.");
        return Ok(());
    }

    let total = api_response["total"].as_u64().unwrap_or(0);
    let total_pages = api_response["total_pages"].as_u64().unwrap_or(1);
    println!(
        "Showing page {} of {total_pages} ({total} total)\n",
        options.page
    );

    print_profile_table(&profiles);
    Ok(())
}

async fn get(id: &str) -> Result<()> {
    let spinner = output::spinner("Fetching profile");
    let api_response = api_get(&format!("/api/profiles/{id}"), &[]).await;
    spinner.finish_and_clear();

    let api_response = api_response?;
    let profile = &api_response["data"];

    output::print_table(
        vec!["Field", "Value"],
        vec![
            vec!["ID".into(), str_val(profile, "id")],
            vec!["Name".into(), str_val(profile, "name")],
            vec!["Gender".into(), str_val(profile, "gender")],
            vec![
                "Gender Probability".into(),
                str_val(profile, "gender_probability"),
            ],
            vec!["Age".into(), str_val(profile, "age")],
            vec!["Age Group".into(), str_val(profile, "age_group")],
            vec!["Country".into(), str_val(profile, "country_name")],
            vec!["Country Code".into(), str_val(profile, "country_id")],
            vec![
                "Country Probability".into(),
                str_val(profile, "country_probability"),
            ],
            vec!["Created At".into(), str_val(profile, "created_at")],
        ],
    );

    Ok(())
}

async fn search(query: &str, page: u32, limit: u32) -> Result<()> {
    let spinner = output::spinner("Searching profiles");
    let page_s = page.to_string();
    let limit_s = limit.to_string();
    let api_response = api_get(
        "/api/profiles/search",
        &[("q", query), ("page", &page_s), ("limit", &limit_s)],
    )
    .await;
    spinner.finish_and_clear();

    let api_response = api_response?;
    let profiles = api_response["data"].as_array().cloned().unwrap_or_default();

    if profiles.is_empty() {
        output::print_success("No profiles found.");
        return Ok(());
    }

    let total = api_response["total"].as_u64().unwrap_or(0);
    let total_pages = api_response["total_pages"].as_u64().unwrap_or(1);
    println!("Showing page {page} of {total_pages} ({total} total)\n");

    print_profile_table(&profiles);
    Ok(())
}

async fn create(name: &str) -> Result<()> {
    let spinner = output::spinner(&format!("Creating profile for '{name}'"));
    let api_response = api_post("/api/profiles", serde_json::json!({ "name": name })).await;
    spinner.finish_and_clear();

    let api_response = api_response?;
    let profile = &api_response["data"];

    output::print_success("Profile created.");
    output::print_table(
        vec!["Field", "Value"],
        vec![
            vec!["ID".into(), str_val(profile, "id")],
            vec!["Name".into(), str_val(profile, "name")],
            vec!["Gender".into(), str_val(profile, "gender")],
            vec!["Age".into(), str_val(profile, "age")],
            vec!["Country".into(), str_val(profile, "country_name")],
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn export(
    format: &str,
    gender: Option<String>,
    country: Option<String>,
    age_group: Option<String>,
    min_age: Option<u8>,
    max_age: Option<u8>,
    sort_by: Option<String>,
    order: Option<String>,
) -> Result<()> {
    let spinner = output::spinner("Exporting profiles");

    let min_age_s = min_age.map(|val| val.to_string());
    let max_age_s = max_age.map(|val| val.to_string());

    let mut query: Vec<(&str, &str)> = vec![("format", format)];
    if let Some(ref gender) = gender {
        query.push(("gender", gender));
    }
    if let Some(ref country) = country {
        query.push(("country_id", country));
    }
    if let Some(ref age_group) = age_group {
        query.push(("age_group", age_group));
    }
    if let Some(ref age) = min_age_s {
        query.push(("min_age", age));
    }
    if let Some(ref age) = max_age_s {
        query.push(("max_age", age));
    }
    if let Some(ref sort) = sort_by {
        query.push(("sort_by", sort));
    }
    if let Some(ref order) = order {
        query.push(("order", order));
    }

    let response = raw_get("/api/profiles/export", &query).await;
    spinner.finish_and_clear();

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
        .and_then(|val| val.to_str().ok())
        .and_then(|val| val.split("filename=").nth(1))
        .map(|filename_str| filename_str.trim_matches('"').to_string())
        .unwrap_or_else(|| "profiles_export.csv".to_string());

    let bytes = response.bytes().await?;
    std::fs::write(&filename, &bytes)?;

    output::print_success(&format!("Exported to {filename}"));
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
            .map(|profile_json| {
                vec![
                    str_val(profile_json, "id"),
                    str_val(profile_json, "name"),
                    str_val(profile_json, "gender"),
                    str_val(profile_json, "age"),
                    str_val(profile_json, "age_group"),
                    str_val(profile_json, "country_name"),
                    str_val(profile_json, "created_at"),
                ]
            })
            .collect(),
    );
}

pub fn str_val(v: &Value, key: &str) -> String {
    match &v[key] {
        Value::String(string_val) => string_val.clone(),
        Value::Number(number_val) => number_val.to_string(),
        Value::Bool(bool_val) => bool_val.to_string(),
        Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}

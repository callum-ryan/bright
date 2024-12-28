use clap::Parser;
use log::debug;
use reqwest::header;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
mod cli;
mod models;

use crate::cli::Cli;
use crate::models::{Entity, Query, Reading};

const GLOWMARKT_AUTH_URI: &str = "https://api.glowmarkt.com/api/v0-1/auth";
const GLOWMARKT_APP_ID: &str = "b0f1b774-a586-4f72-9edd-27ead8aa7a8d";

#[tokio::main]
async fn main() {
    env_logger::init();

    let cli = Cli::parse();
    let client = reqwest::Client::new();
    let api_token = get_api_token(&client, &cli).await;
    let headers = setup_headers(api_token);
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("Failed to build client");

    if let Ok(entities) = get_entities(&client).await {
        process_entities(&client, entities).await;
    } else {
        eprintln!("Failed to fetch entities");
    }
}

async fn get_api_token(client: &reqwest::Client, cli: &Cli) -> String {
    if let Some(cache_file) = &cli.token_cache_file {
        let token = read_local_token(cache_file);
        if check_token_expiry(&token) {
            token.get("token").unwrap().as_str().unwrap().to_string()
        } else {
            refresh_and_cache_token(client, cache_file, &cli.gm_username, &cli.gm_password).await
        }
    } else {
        let new_token = get_auth(client, &cli.gm_username, &cli.gm_password).await;
        new_token
            .get("token")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    }
}

fn setup_headers(api_token: String) -> header::HeaderMap {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "applicationId",
        header::HeaderValue::from_static(GLOWMARKT_APP_ID),
    );
    headers.insert(
        "Content-Type",
        header::HeaderValue::from_static("application/json"),
    );
    headers.insert("token", api_token.parse().unwrap());
    headers
}

async fn refresh_and_cache_token(
    client: &reqwest::Client,
    cache_file: &str,
    username: &str,
    password: &str,
) -> String {
    let new_token = get_auth(client, username, password).await;
    let serialized_token = serde_json::to_string(&new_token)
        .expect("Failed to serialize JSON token")
        .into_bytes();
    write_auth_to_file(cache_file, &serialized_token);
    new_token
        .get("token")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

fn read_local_token(token_path: &str) -> HashMap<String, serde_json::Value> {
    let file = File::open(token_path)
        .unwrap_or_else(|_| panic!("unable to read local token from {token_path}"));
    let result: HashMap<String, serde_json::Value> =
        serde_json::from_reader(file).expect("unable to parse JSON from token file");
    result
}

fn write_auth_to_file(token_path: &str, token_bytes: &[u8]) {
    let mut file = File::create(token_path).expect("failed to create token file");
    file.write_all(token_bytes)
        .expect("failed to write to file");
}

fn check_token_expiry(auth_map: &HashMap<String, serde_json::Value>) -> bool {
    let expiry = auth_map
        .get("exp")
        .expect("token does not contain an expiry")
        .as_u64()
        .unwrap();

    let exp_duration = Duration::new(expiry, 0);

    let start = SystemTime::now();
    let now = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    let diff = (exp_duration - now).as_secs();

    debug!("current time: {now:?}");
    debug!("token expiry: {expiry:?}");
    debug!("seconds remaining: {diff:?}");

    diff > 500
}


async fn get_auth(
    client: &reqwest::Client,
    username: &str,
    password: &str,
) -> HashMap<String, serde_json::Value> {
    let body: HashMap<&str, &str> =
        HashMap::from_iter([("username", username), ("password", password)]);

    let response = client
        .post(GLOWMARKT_AUTH_URI)
        .header("applicationId", GLOWMARKT_APP_ID)
        .json(&body)
        .send()
        .await
        .unwrap();

    response
        .json::<HashMap<String, serde_json::Value>>()
        .await
        .unwrap()
}


async fn get_entities(client: &reqwest::Client) -> Result<Vec<Entity>, reqwest::Error> {
    let url = "https://api.glowmarkt.com/api/v0-1/virtualentity";
    let response = client.get(url).send().await?;

    let entities = response.json::<Vec<Entity>>().await?;

    Ok(entities)
}


async fn process_entities(client: &reqwest::Client, entities: Vec<Entity>) {
    for entity in entities {
        debug!("{:?}", entity);
        for resource in entity.resources {
            let query = Query {
                from: "2024-12-01T00:00:00".to_string(),
                to: "2024-12-10T00:00:00".to_string(),
                period: "PT30M".to_string(),
                function: "sum".to_string(),
            };
            if let Err(e) = get_readings_for_resource(client, &resource.resource_id, query).await {
                eprintln!(
                    "Failed to fetch readings for resource {}: {:?}",
                    resource.resource_id, e
                );
            }
        }
    }
}


async fn get_readings_for_resource(
    client: &reqwest::Client,
    resource_id: &str,
    query: Query,
) -> Result<Reading, reqwest::Error> {
    let url = format!("https://api.glowmarkt.com/api/v0-1/resource/{resource_id}/readings?");
    let response = client
        .get(url)
        .query(&[
            ("from", query.from),
            ("to", query.to),
            ("period", query.period),
            ("function", query.function),
        ])
        .send()
        .await?;

    let readings = response.json::<Reading>().await?;
    Ok(readings)
}

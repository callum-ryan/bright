use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, TimeZone};
use clap::Parser;
use influxdb::InfluxDbWriteable;
use log::{debug, error, info};
use reqwest::header;

mod cli;
mod models;

use crate::cli::Cli;
use crate::models::{Entity, Reading, ResourceQuery};

const GLOWMARKT_AUTH_URI: &str = "https://api.glowmarkt.com/api/v0-1/auth";
const GLOWMARKT_APP_ID: &str = "b0f1b774-a586-4f72-9edd-27ead8aa7a8d";
const DEFAULT_PERIOD: &str = "PT30M";
const DEFAULT_FUNCTION: &str = "sum";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    info!("Starting Glowmarkt API client");

    let cli = Cli::parse();
    let client = reqwest::Client::new();
    let api_token = get_api_token(&client, &cli).await?;
    let headers = setup_headers(api_token)?;
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let (start, end) = get_date_range(&cli)?;
    info!("Requesting data from GlowMarkt for {:?} - {:?}", start, end);

    let batches = create_date_batches(start, end);
    let readings = process_entities(&client, get_entities(&client).await?, &batches).await?;

    if !readings.is_empty() {
        let influx_client =
            influxdb::Client::new(cli.influx_uri, cli.influx_database).with_token(cli.influx_token);
        influx_client.query(readings).await?;
    }

    Ok(())
}

fn get_date_range(
    cli: &Cli,
) -> Result<(DateTime<Local>, DateTime<Local>), Box<dyn std::error::Error>> {
    match (cli.start_date, cli.end_date) {
        (Some(start), Some(end)) => Ok((start, end)),
        (None, None) => {
            let now = Local::now();
            Ok((
                (now - chrono::Duration::days(10))
                    .with_time(chrono::NaiveTime::MIN)
                    .unwrap(),
                now,
            ))
        }
        _ => Err("Either both dates must be provided, or neither.".into()),
    }
}

fn create_date_batches(
    start: DateTime<Local>,
    end: DateTime<Local>,
) -> Vec<(DateTime<Local>, DateTime<Local>)> {
    let mut batches = Vec::new();
    let mut start_date = start;
    let mut end_date = min_dates(start_date + chrono::Duration::days(10), end);

    batches.push((start_date, end_date));

    while end_date < end {
        start_date += chrono::Duration::days(10);
        end_date = min_dates(start_date + chrono::Duration::days(10), end);
        batches.push((start_date, end_date));
    }

    batches
}

fn min_dates<Tz: TimeZone>(d1: DateTime<Tz>, d2: DateTime<Tz>) -> DateTime<Tz> {
    if d1 < d2 {
        d1
    } else {
        d2
    }
}

async fn get_api_token(
    client: &reqwest::Client,
    cli: &Cli,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(cache_file) = &cli.token_cache_file {
        if let Ok(token) = read_local_token(cache_file) {
            if check_token_expiry(&token)? {
                return Ok(token
                    .get("token")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string());
            }
        }
        refresh_and_cache_token(client, cache_file, &cli.gm_username, &cli.gm_password).await
    } else {
        let new_token = get_auth(client, &cli.gm_username, &cli.gm_password).await?;
        Ok(new_token
            .get("token")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string())
    }
}

fn setup_headers(api_token: String) -> Result<header::HeaderMap, Box<dyn std::error::Error>> {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "applicationId",
        header::HeaderValue::from_static(GLOWMARKT_APP_ID),
    );
    headers.insert(
        "Content-Type",
        header::HeaderValue::from_static("application/json"),
    );
    headers.insert("token", api_token.parse()?);
    Ok(headers)
}

async fn refresh_and_cache_token(
    client: &reqwest::Client,
    cache_file: &str,
    username: &str,
    password: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let new_token = get_auth(client, username, password).await?;
    let serialized_token = serde_json::to_vec(&new_token)?;
    write_auth_to_file(cache_file, &serialized_token)?;
    Ok(new_token
        .get("token")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string())
}

fn read_local_token(
    token_path: &str,
) -> Result<HashMap<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let file = File::open(token_path)?;
    let result: HashMap<String, serde_json::Value> = serde_json::from_reader(file)?;
    Ok(result)
}

fn write_auth_to_file(token_path: &str, token_bytes: &[u8]) -> Result<(), std::io::Error> {
    let mut file = File::create(token_path)?;
    file.write_all(token_bytes)?;
    Ok(())
}

fn check_token_expiry(
    auth_map: &HashMap<String, serde_json::Value>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let expiry = auth_map
        .get("exp")
        .and_then(serde_json::Value::as_u64)
        .ok_or("Token does not contain an expiry")?;
    let exp_duration = Duration::new(expiry, 0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let diff = (exp_duration - now).as_secs();

    debug!("Current time: {:?}", now);
    debug!("Token expiry: {:?}", expiry);
    debug!("Seconds remaining: {:?}", diff);

    Ok(diff > 500)
}

async fn get_auth(
    client: &reqwest::Client,
    username: &str,
    password: &str,
) -> Result<HashMap<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let body = HashMap::from([("username", username), ("password", password)]);
    let response = client
        .post(GLOWMARKT_AUTH_URI)
        .header("applicationId", GLOWMARKT_APP_ID)
        .json(&body)
        .send()
        .await?;

    Ok(response
        .json::<HashMap<String, serde_json::Value>>()
        .await?)
}

async fn get_entities(client: &reqwest::Client) -> Result<Vec<Entity>, reqwest::Error> {
    let url = "https://api.glowmarkt.com/api/v0-1/virtualentity";
    let response = client.get(url).send().await?;
    let entities = response.json::<Vec<Entity>>().await?;
    Ok(entities)
}

async fn process_entities(
    client: &reqwest::Client,
    entities: Vec<Entity>,
    date_batches: &[(DateTime<Local>, DateTime<Local>)],
) -> Result<Vec<influxdb::WriteQuery>, Box<dyn std::error::Error>> {
    let mut influx = Vec::new();

    for entity in entities {
        debug!("Processing entity: {:?}", entity);
        for resource in entity.resources {
            for (from, to) in date_batches {
                debug!("{:?} - {:?}", from, to);
                let query = ResourceQuery {
                    from: from.format("%Y-%m-%dT%H:%M:%S").to_string(),
                    to: to.format("%Y-%m-%dT%H:%M:%S").to_string(),
                    period: DEFAULT_PERIOD.to_string(),
                    function: DEFAULT_FUNCTION.to_string(),
                };

                match get_readings_for_resource(client, &resource.resource_id, query).await {
                    Ok(readings) => {
                        for m in readings.to_influx() {
                            influx.push(m.into_query("glowmarkt"));
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to fetch readings for resource {}: {:?}",
                            resource.resource_id, e
                        );
                    }
                }
            }
        }
    }

    Ok(influx)
}

async fn get_readings_for_resource(
    client: &reqwest::Client,
    resource_id: &str,
    query: ResourceQuery,
) -> Result<Reading, Box<dyn std::error::Error>> {
    let url = format!("https://api.glowmarkt.com/api/v0-1/resource/{resource_id}/readings?");
    let response = client
        .get(&url)
        .query(&[
            ("from", &query.from),
            ("to", &query.to),
            ("period", &query.period),
            ("function", &query.function),
        ])
        .send()
        .await?;

    let response_text = response.text().await?;
    debug!(
        "Raw response for {} and query {:?}: {:?}",
        &url, &query, response_text
    );

    Ok(serde_json::from_str::<Reading>(&response_text)?)
}

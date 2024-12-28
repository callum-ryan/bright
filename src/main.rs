use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::Parser;
use log::debug;
use reqwest::header;
mod cli;
mod models;
use chrono::{DateTime, Local, TimeZone};
use influxdb::InfluxDbWriteable;

use crate::cli::Cli;
use crate::models::{Entity, Reading, ResourceQuery};

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

    let mut batches = Vec::new();

    if (cli.end_date - cli.start_date).num_days() > 10 {
        debug!("requested more than 10 days of data, chunking requests");
        let mut start_date = cli.start_date;
        let mut end_date = cli.start_date + chrono::Duration::days(10);

        batches.push((start_date, end_date));

        while end_date < cli.end_date {
            start_date += chrono::Duration::days(10);
            end_date = min_dates(start_date + chrono::Duration::days(10), cli.end_date);
            batches.push((start_date, end_date));
        }
    } else {
        batches.push((cli.start_date, cli.end_date));
    }

    let readings = if let Ok(entities) = get_entities(&client).await {
        process_entities(&client, entities, &batches).await
    } else {
        eprintln!("Failed to fetch entities");
        Ok(Vec::new())
    };

    let readings = if let Ok(readings) = readings {
        readings
    } else {
        eprintln!("Failed to fetch readings");
        Vec::new()
    };

    let influx_client =
        influxdb::Client::new(cli.influx_uri, cli.influx_database).with_token(cli.influx_token);

    if !readings.is_empty() {
        influx_client.query(readings).await.unwrap();
    }
}

fn min_dates<Tz: TimeZone>(d1: DateTime<Tz>, d2: DateTime<Tz>) -> DateTime<Tz> {
    let d1_unix = d1.timestamp();
    let d2_unix = d2.timestamp();
    match d1_unix.cmp(&d2_unix) {
        std::cmp::Ordering::Less => d1,
        std::cmp::Ordering::Greater => d2,
        std::cmp::Ordering::Equal => d1,
    }
}

async fn get_api_token(client: &reqwest::Client, cli: &Cli) -> String {
    if let Some(cache_file) = &cli.token_cache_file {
        let token = read_local_token(cache_file);
        if let Ok(token) = token {
            if check_token_expiry(&token) {
                token.get("token").unwrap().as_str().unwrap().to_string()
            } else {
                refresh_and_cache_token(client, cache_file, &cli.gm_username, &cli.gm_password)
                    .await
            }
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

fn read_local_token(
    token_path: &str,
) -> Result<HashMap<String, serde_json::Value>, std::io::Error> {
    let file = File::open(token_path)?;
    let result: HashMap<String, serde_json::Value> =
        serde_json::from_reader(file).expect("unable to parse JSON from token file");
    Ok(result)
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

async fn process_entities(
    client: &reqwest::Client,
    entities: Vec<Entity>,
    date_batches: &Vec<(DateTime<Local>, DateTime<Local>)>,
) -> Result<Vec<influxdb::WriteQuery>, reqwest::Error> {
    let mut influx = Vec::new();

    for entity in entities {
        debug!("{:?}", entity);
        for resource in entity.resources {
            for (from, to) in date_batches {
                debug!("{:?} - {:?}", from, to);
                let query = ResourceQuery {
                    from: format!("{}", from.format("%Y-%m-%dT%H:%M:%S")),
                    to: format!("{}", to.format("%Y-%m-%dT%H:%M:%S")),
                    period: "PT30M".to_string(),
                    function: "sum".to_string(),
                };

                match get_readings_for_resource(client, &resource.resource_id, query).await {
                    Ok(readings) => {
                        for m in readings.to_influx() {
                            influx.push(m.into_query("glowmarkt"));
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to fetch readings for resource {}: {:?}",
                            resource.resource_id, e
                        );
                    }
                };
            }
        }
    }
    Ok(influx)
}

async fn get_readings_for_resource(
    client: &reqwest::Client,
    resource_id: &str,
    query: ResourceQuery,
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

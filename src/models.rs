use std::collections::HashMap;

use chrono::TimeZone;
use influxdb::InfluxDbWriteable;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Resource {
    name: String,
    pub resource_id: String,
    resource_type_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Entity {
    application_id: String,
    postal_code: String,
    pub resources: Vec<Resource>,
    owner_id: String,
    ve_id: String,
    clone: bool,
    ve_children: Vec<String>,
    attributes: HashMap<String, HashMap<String, String>>,
    ve_type_id: String,
    updated_at: String,
    created_at: String,
    active: bool,
    name: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Reading {
    status: String,
    name: String,
    resource_type_id: String,
    resource_id: String,
    query: ResourceQuery,
    data: Vec<Vec<f64>>,
    units: String,
    classifier: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceQuery {
    pub from: String,
    pub to: String,
    pub period: String,
    pub function: String,
}

#[derive(InfluxDbWriteable, Clone, Default)]
pub struct InfluxValue {
    time: chrono::DateTime<chrono::Utc>,
    value: f64,
    #[influxdb(tag)]
    classifier: String,
    #[influxdb(tag)]
    measurement: String,
}

impl Reading {
    pub fn to_influx(&self) -> Vec<InfluxValue> {
        self.data
            .iter()
            .map(|v| InfluxValue {
                time: chrono::Utc.timestamp_opt(v[0] as i64, 0).unwrap(),
                value: v[1],
                classifier: self.classifier.clone(),
                measurement: self.classifier.clone(),
            })
            .collect::<Vec<InfluxValue>>()
    }
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum GetReadingsError {
    #[error("Request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Failed to parse response JSON: {0}")]
    Parse(#[from] serde_json::Error),
}

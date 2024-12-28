use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    name: String,
    pub resource_id: String,
    resource_type_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
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
pub struct Reading {
    status: String,
    name: String,
    resource_type_id: String,
    resource_id: String,
    query: Query,
    data: Vec<Vec<f64>>,
    units: String,
    classifier: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub from: String,
    pub to: String,
    pub period: String,
    pub function: String,
}

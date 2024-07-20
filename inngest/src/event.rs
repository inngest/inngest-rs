use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use std::default::Default;

#[derive(Debug, Deserialize, Serialize)]
pub struct Event {
    pub id: Option<String>,
    pub name: String,
    // TODO: data should be a generic
    pub timestamp: Option<i64>,
    pub version: Option<String>,
}

impl Default for Event {
    fn default() -> Self {
        Event {
            id: None,
            name: String::new(),
            timestamp: None,
            version: None
        }
    }
}

impl Event {
    fn new() -> Self {
        Event::default()
    }
}

pub async fn send_event(event: Event) -> Result<(), String> {
    let client = reqwest::Client::new();

    // Take the value where the content is
    let payload = &json!(event)["value"];

    // TODO: make the result return something properly
    match client
        .post("http://127.0.0.1:8288/e/test")
        .json(payload)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err("failed to send event".to_string()),
    }
}

pub async fn send_events(events: &[&Event]) -> Result<(), String> {
    let client = reqwest::Client::new();

    let payload: Vec<Value> = events
        .iter()
        .map(|evt| json!(evt)["value"].clone())
        .collect();

    // TODO: make the result return something properly
    match client
        .post("http://127.0.0.1:8288/e/test")
        .json(&payload)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err("failed to send events".to_string()),
    }
}

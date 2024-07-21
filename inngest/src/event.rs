use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub trait InngestEvent: Serialize + for<'a> Deserialize<'a> + Debug + 'static {}
impl<T: Serialize + for<'a> Deserialize<'a> + Debug + 'static> InngestEvent for T {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Event<T>
where
    T: 'static,
{
    pub id: Option<String>,
    pub name: String,
    pub data: T,
    pub timestamp: Option<i64>,
    pub version: Option<String>,
}

impl<T> Event<T>
where
    T: InngestEvent,
{
    pub fn new(name: &str, data: T) -> Self {
        Event {
            id: None,
            name: name.to_string(),
            data,
            timestamp: None,
            version: None,
        }
    }

    pub fn id(&mut self, id: &str) -> &mut Self {
        self.id = Some(id.to_string());
        self
    }

    pub fn timestamp(&mut self, ts: i64) -> &mut Self {
        self.timestamp = Some(ts);
        self
    }

    pub fn version(&mut self, v: &str) -> &mut Self {
        self.version = Some(v.to_string());
        self
    }
}

pub async fn send_event<T: InngestEvent>(event: &Event<T>) -> Result<(), String> {
    let client = reqwest::Client::new();
    let payload = json!(event);

    // TODO: make the result return something properly
    match client
        .post("http://127.0.0.1:8288/e/test")
        .json(&payload)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err("failed to send event".to_string()),
    }
}

pub async fn send_events<T: InngestEvent>(events: &[&Event<T>]) -> Result<(), String> {
    let client = reqwest::Client::new();
    let payload: Vec<Value> = events.iter().map(|evt| json!(evt)).collect();

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

use serde::{Serialize, Deserialize};
use serde_json::{json, Value};

#[derive(Debug, Serialize, Deserialize)]
pub struct Event<T>
where T: 'static
{
    pub id: Option<String>,
    pub name: String,
    pub data: T,
    pub timestamp: Option<i64>,
    pub version: Option<String>,
}

// impl Event {
//     fn new() -> Self {
//         Event::default()
//     }
// }

pub async fn send_event<T: Serialize + for<'a> Deserialize<'a>>(event: &Event<T>) -> Result<(), String> {
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

pub async fn send_events<T: Serialize + for<'a> Deserialize<'a>>(events: &[&Event<T>]) -> Result<(), String> {
    let client = reqwest::Client::new();
    let payload: Vec<Value> = events
        .iter()
        .map(|evt| json!(evt))
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

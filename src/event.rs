use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Event<T, U> {
    pub id: Option<String>,
    pub name: String,
    pub data: T,
    pub user: Option<U>,
    pub ts: u32,
}

impl<T: Default, U> Default for Event<T, U> {
    fn default() -> Self {
        Event {
            id: None,
            name: String::new(),
            data: T::default(),
            user: None,
            ts: 0,
        }
    }
}

pub async fn send_event<D: Serialize, U: Serialize>(event: &Event<D, U>) -> Result<(), String> {
    let client = reqwest::Client::new();

    // TODO: make the result return something properly
    match client
        .post("http://127.0.0.1:8288/e/test")
        .json(&event)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err("failed to send event".to_string()),
    }
}

pub async fn send_events<D: Serialize, U: Serialize>(events: &[Event<D, U>]) -> Result<(), String> {
    let client = reqwest::Client::new();

    // TODO: make the result return something properly
    match client
        .post("http://127.0.0.1:8288/e/test")
        .json(events)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err("failed to send events".to_string()),
    }
}

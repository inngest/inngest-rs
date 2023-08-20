use std::any::Any;

#[typetag::serde(tag = "type")]
pub trait Event {
    fn id(&self) -> Option<String>;
    fn name(&self) -> String;
    fn data(&self) -> &dyn Any;
    fn user(&self) -> Option<&dyn Any>;
    fn timestamp(&self) -> Option<u64>;
    fn version(&self) -> Option<String>;
}

pub async fn send_event(event: &dyn Event) -> Result<(), String> {
    let client = reqwest::Client::new();

    // TODO: make the result return something properly
    match client
        .post("http://127.0.0.1:8288/e/test")
        .json(event)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err("failed to send event".to_string()),
    }
}

pub async fn send_events(events: &[&dyn Event]) -> Result<(), String> {
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

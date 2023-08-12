use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// #[derive(Debug, Clone, Deserialize, Serialize)]
pub trait ServableFunction {
    fn slug(&self) -> String;
    fn name(&self) -> String;
    fn trigger(&self) -> Trigger;
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Function {
    id: String,
    name: String,
    triggers: Vec<Trigger>,
    steps: HashMap<String, Step>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Step {
    id: String,
    name: String,
    runtime: StepRuntime,
    retries: StepRetry,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StepRuntime {
    url: String,
    #[serde(rename = "type")]
    method: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StepRetry {
    attempts: u8,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Trigger {
    Event(EventTrigger),
    Cron(CronTrigger),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventTrigger {
    event: String,
    expression: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CronTrigger {
    cron: String,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServableFunction {
    slug: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventTrigger {
    event: String,
    expression: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CronTrigger {
    cron: String,
}

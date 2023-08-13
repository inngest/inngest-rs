use std::{any::Any, collections::HashMap};

use serde::{Deserialize, Serialize};
use slug::slugify;

#[derive(Deserialize)]
pub struct Input<T> {
    event: T,
    events: Vec<T>,
    ctx: InputCtx,
}

#[derive(Deserialize)]
pub struct InputCtx {
    fn_id: String,
    run_id: String,
    step_id: String,
}

type SdkFunction<T> = fn(Input<T>) -> Result<dyn Any, String>;

#[derive(Debug, Clone)]
pub struct FunctionOps {
    id: Option<String>,
    name: String,
    retries: u8,
}

#[derive(Debug, Clone)]
pub struct ServableFunction {
    opts: FunctionOps,
    trigger: Trigger,
    // func: SdkFunction,
}

impl ServableFunction {
    pub fn slug(&self) -> String {
        match &self.opts.id {
            Some(id) => id.clone(),
            None => slugify(self.name()),
        }
    }

    pub fn name(&self) -> String {
        self.opts.name.clone()
    }
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Trigger {
    Event(EventTrigger),
    Cron(CronTrigger),
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

pub fn create_function<T>(opts: FunctionOps, trigger: Trigger) -> ServableFunction {
    ServableFunction { opts, trigger }
}

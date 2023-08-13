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
    pub id: String,
    pub name: String,
    pub triggers: Vec<Trigger>,
    pub steps: HashMap<String, Step>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Step {
    pub id: String,
    pub name: String,
    pub runtime: StepRuntime,
    pub retries: StepRetry,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StepRuntime {
    pub url: String,
    #[serde(rename = "type")]
    pub method: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StepRetry {
    pub attempts: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Trigger {
    EventTrigger {
        event: String,
        expression: Option<String>,
    },
    CronTrigger {
        cron: String,
    },
}

pub fn create_function<T>(opts: FunctionOps, trigger: Trigger) -> ServableFunction {
    ServableFunction { opts, trigger }
}

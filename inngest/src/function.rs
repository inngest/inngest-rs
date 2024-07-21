use crate::event::Event;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slug::slugify;
use std::{any::Any, collections::HashMap, fmt::Debug};

#[derive(Deserialize)]
pub struct Input<T>
where
    T: 'static,
{
    pub event: Event<T>,
    pub events: Vec<Event<T>>,
    pub ctx: InputCtx,
}

#[derive(Deserialize)]
pub struct InputCtx {
    pub fn_id: String,
    pub run_id: String,
    pub step_id: String,
}

#[derive(Debug, Clone)]
pub struct FunctionOps {
    pub id: String,
    pub name: Option<String>,
    pub retries: u8,
}

impl Default for FunctionOps {
    fn default() -> Self {
        FunctionOps {
            id: String::new(),
            name: None,
            retries: 3,
        }
    }
}

pub struct ServableFn<T>
where
    T: Serialize + for<'a> Deserialize<'a> + 'static,
{
    pub opts: FunctionOps,
    pub trigger: Trigger,
    pub func: fn(Input<T>) -> Result<Box<dyn Any>, String>,
}

impl<T> ServableFn<T>
where
    T: Serialize + for<'a> Deserialize<'a> + 'static,
{
    // TODO: prepend app_id
    pub fn slug(&self) -> String {
        slugify(self.opts.id.clone())
    }

    pub fn trigger(&self) -> Trigger {
        self.trigger.clone()
    }

    pub fn event(&self, data: Value) -> Option<Event<T>> {
        match serde_json::from_value::<Event<T>>(data) {
            Ok(val) => Some(val),
            Err(_) => None,
        }
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

pub fn create_function<T: Serialize + for<'a> Deserialize<'a> + 'static>(
    opts: FunctionOps,
    trigger: Trigger,
    func: fn(Input<T>) -> Result<Box<dyn Any>, String>,
) -> ServableFn<T> {
    ServableFn {
        opts,
        trigger,
        func,
    }
}

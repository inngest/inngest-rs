use crate::event::Event;
use serde::{Deserialize, Serialize};
use slug::slugify;
use std::{any::Any, collections::HashMap, fmt::Debug};

pub trait ServableFunction {
    fn slug(&self) -> String;
    fn name(&self) -> String;
    fn trigger(&self) -> Trigger;
}

impl Debug for dyn ServableFunction + Send + Sync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServableFn")
            .field("name", &self.name())
            .field("slug", &self.slug())
            .field("trigger", &self.trigger())
            .finish()
    }
}

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
    pub id: Option<String>,
    pub name: String,
    pub retries: u8,
}

impl Default for FunctionOps {
    fn default() -> Self {
        FunctionOps {
            id: None,
            name: String::new(),
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
    pub func: Box<dyn Fn(Input<T>) -> Result<Box<dyn Any>, String> + Send + Sync>,
}

impl<T> ServableFunction for ServableFn<T>
where
    T: Serialize + for<'a> Deserialize<'a> + 'static,
{
    fn slug(&self) -> String {
        match &self.opts.id {
            Some(id) => id.clone(),
            None => slugify(self.name()),
        }
    }

    fn name(&self) -> String {
        self.opts.name.clone()
    }

    fn trigger(&self) -> Trigger {
        self.trigger.clone()
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
    func: Box<dyn Fn(Input<T>) -> Result<Box<dyn Any>, String> + Send + Sync>,
) -> Box<dyn ServableFunction + Sync + Send> {
    let servable = ServableFn {
        opts,
        trigger,
        func,
    };

    Box::new(servable)
}

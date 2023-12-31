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
pub struct Input<T> {
    pub event: T,
    pub events: Vec<T>,
    pub ctx: InputCtx,
}

#[derive(Deserialize)]
pub struct InputCtx {
    pub fn_id: String,
    pub run_id: String,
    pub step_id: String,
}

type SdkFunction = dyn Fn(Input<&dyn Event>) -> Result<Box<dyn Any>, String> + Send + Sync;

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

pub struct ServableFn {
    pub opts: FunctionOps,
    pub trigger: Trigger,
    pub func: Box<SdkFunction>,
}

impl ServableFunction for ServableFn {
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

pub fn create_function(
    opts: FunctionOps,
    trigger: Trigger,
    func: Box<SdkFunction>,
) -> Box<dyn ServableFunction + Sync + Send> {
    let servable = ServableFn {
        opts,
        trigger,
        func,
    };

    Box::new(servable)
}

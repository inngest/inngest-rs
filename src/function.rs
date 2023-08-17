use serde::{Deserialize, Serialize};
use slug::slugify;
use std::{any::Any, collections::HashMap, fmt::Debug, sync::Arc};

pub trait ServableFunction {
    type T: Serialize;

    fn slug(&self) -> String;
    fn name(&self) -> String;
    fn trigger(&self) -> Trigger;
    fn invoke(&self, input: Input<Self::T>) -> Result<Box<dyn Any>, String>;
}

impl Debug for dyn ServableFunction<T = ()> + Send + Sync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServableFn")
            .field("name", &self.name())
            .field("slug", &self.slug())
            .field("trigger", &self.trigger())
            .finish()
    }
}

#[derive(Deserialize, Serialize)]
pub struct Input<T: Serialize> {
    pub event: T,
    pub events: Vec<T>,
    pub ctx: InputCtx,
}

#[derive(Deserialize, Serialize)]
pub struct InputCtx {
    pub fn_id: String,
    pub run_id: String,
    // pub step_id: String,
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

#[derive(Clone)]
pub struct ServableFn<T: Serialize> {
    pub opts: FunctionOps,
    pub trigger: Trigger,
    pub event: T,
    pub func: Arc<dyn Fn(Input<T>) -> Result<Box<dyn Any>, String> + Send + Sync>,
}

impl<T: Serialize> ServableFunction for ServableFn<T> {
    type T = T;

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

    fn invoke(&self, input: Input<Self::T>) -> Result<Box<dyn Any>, String> {
        (self.func)(input)
    }
}

pub fn create_function<T: Serialize + Default>(
    opts: FunctionOps,
    trigger: Trigger,
    func: impl Fn(Input<T>) -> Result<Box<dyn Any>, String> + Send + Sync + 'static,
) -> impl ServableFunction {
    ServableFn {
        opts,
        trigger,
        event: T::default(),
        func: Arc::new(func),
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

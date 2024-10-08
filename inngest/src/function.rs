use crate::{
    event::{Event, InngestEvent},
    step_tool::Step as StepTool,
};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slug::slugify;
use std::{collections::HashMap, fmt::Debug, future::Future};

// NOTE: should T have Copy trait too?
// so it can do something like `input.event` without moving.
// but the benefit vs effort might be too much for users.
pub struct Input<T: 'static> {
    pub event: Event<T>,
    pub events: Vec<Event<T>>,
    pub ctx: InputCtx,
}

pub struct InputCtx {
    pub env: String,
    pub fn_id: String,
    pub run_id: String,
    pub step_id: String,
    pub attempt: u8,
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

type Func<T, E> =
    dyn Fn(Input<T>, StepTool) -> BoxFuture<'static, Result<Value, E>> + Send + Sync + 'static;

pub struct ServableFn<T: 'static, E> {
    pub opts: FunctionOps,
    pub trigger: Trigger,
    pub func: Box<Func<T, E>>,
}

impl<T: InngestEvent, E> Debug for ServableFn<T, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServableFn")
            .field("id", &self.opts.id)
            .field("trigger", &self.trigger())
            .finish()
    }
}

impl<T, E> ServableFn<T, E> {
    // TODO: prepend app_id
    // TODO: slugify id
    pub fn slug(&self) -> String {
        slugify(self.opts.id.clone())
    }

    pub fn trigger(&self) -> Trigger {
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

pub fn create_function<T: 'static, E, F>(
    opts: FunctionOps,
    trigger: Trigger,
    func: impl Fn(Input<T>, StepTool) -> F + Send + Sync + 'static,
) -> ServableFn<T, E>
where
    F: Future<Output = Result<Value, E>> + Send + Sync + 'static,
{
    use futures::future::FutureExt;

    ServableFn {
        opts,
        trigger,
        func: Box::new(move |input, step| func(input, step).boxed()),
    }
}

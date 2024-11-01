use crate::{
    event::{Event, InngestEvent},
    step_tool::Step as StepTool,
};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slug::slugify;
use std::{collections::HashMap, fmt::Debug};

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
pub struct FunctionOpts {
    pub id: String,
    pub name: Option<String>,
    pub retries: u8,
}

impl Default for FunctionOpts {
    fn default() -> Self {
        FunctionOpts {
            id: String::new(),
            name: None,
            retries: 3,
        }
    }
}

impl FunctionOpts {
    pub fn new(id: &str) -> Self {
        FunctionOpts {
            id: id.to_string(),
            ..Default::default()
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }
}

type Func<T, E> =
    dyn Fn(Input<T>, StepTool) -> BoxFuture<'static, Result<Value, E>> + Send + Sync + 'static;

pub struct ServableFn<T: 'static, E> {
    pub(crate) app_id: String,
    pub opts: FunctionOpts,
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
    pub fn slug(&self) -> String {
        format!("{}-{}", &self.app_id, slugify(self.opts.id.clone()))
    }

    pub fn name(&self) -> String {
        match self.opts.name.clone() {
            Some(name) => name,
            None => self.slug(),
        }
    }

    pub fn trigger(&self) -> Trigger {
        self.trigger.clone()
    }

    pub fn function(&self, serve_origin: &str, serve_path: &str) -> Function {
        let id = format!("{}-{}", &self.app_id, slugify(self.opts.id.clone()));
        let name = match self.opts.name.clone() {
            Some(name) => name,
            None => id.clone(),
        };

        let mut steps = HashMap::new();
        steps.insert(
            "step".to_string(),
            Step {
                id: "step".to_string(),
                name: "step".to_string(),
                runtime: StepRuntime {
                    url: format!("{}{}?fnId={}&step=step", serve_origin, serve_path, &id),
                    method: "http".to_string(),
                },
                retries: StepRetry {
                    attempts: self.opts.retries,
                },
            },
        );

        Function {
            id,
            name,
            triggers: vec![self.trigger.clone()],
            steps,
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

impl Trigger {
    pub fn event(name: &str) -> Self {
        Trigger::EventTrigger {
            event: name.to_string(),
            expression: None,
        }
    }

    #[allow(unused_variables)]
    pub fn expr(&self, exp: &str) -> Self {
        match self {
            Trigger::EventTrigger { event, expression } => Trigger::EventTrigger {
                event: event.clone(),
                expression: Some(exp.to_string()),
            },
            Trigger::CronTrigger { cron } => Trigger::CronTrigger { cron: cron.clone() },
        }
    }

    pub fn cron(cron: &str) -> Self {
        Trigger::CronTrigger {
            cron: cron.to_string(),
        }
    }
}

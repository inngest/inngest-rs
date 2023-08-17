pub mod axum;

use crate::function::ServableFunction;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, default::Default};

#[derive(Debug)]
pub struct Handler {
    app_name: String,
    funcs: Vec<Box<dyn ServableFunction + Sync + Send>>,
}

impl Handler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_name(&mut self, name: &str) {
        self.app_name = name.to_string()
    }

    pub fn register_fn(&mut self, func: impl ServableFunction + 'static + Sync + Send) {
        self.funcs.push(Box::new(func));
    }

    // pub fn register_fns(&mut self, funcs: &[ServableFunction]) {
    //     self.funcs.extend_from_slice(funcs)
    // }
}

impl Default for Handler {
    fn default() -> Self {
        Handler {
            app_name: "InngestApp".to_string(),
            funcs: vec![],
        }
    }
}

#[derive(Deserialize)]
pub struct InvokeQuery {
    #[serde(rename = "fnId")]
    fn_id: String,
    // step: String,
}

#[derive(Deserialize)]
pub struct InvokeBody<T> {
    ctx: InvokeBodyCtx,
    event: T,
    events: Vec<T>,
    steps: HashMap<String, Value>,
    use_api: bool,
}

#[derive(Deserialize)]
pub struct InvokeBodyCtx {
    attempt: u8,
    env: String,
    fn_id: String,
    run_id: String,
    stack: InvokeBodyCtxStack,
    step_id: String,
}

#[derive(Deserialize)]
pub struct InvokeBodyCtxStack {
    current: u16,
    stack: Vec<String>,
}

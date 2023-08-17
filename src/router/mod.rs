pub mod axum;

use crate::function::ServableFunction;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, default::Default};

pub struct Handler<F: ServableFunction> {
    app_name: String,
    funcs: Vec<Box<F>>,
}

impl<F: ServableFunction> Handler<F> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_name(&mut self, name: &str) {
        self.app_name = name.to_string()
    }

    pub fn register_fn(&mut self, func: F) {
        self.funcs.push(Box::new(func));
    }

    // pub fn register_fns(&mut self, funcs: &[ServableFunction]) {
    //     self.funcs.extend_from_slice(funcs)
    // }
}

impl<F: ServableFunction> Default for Handler<F> {
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

#[derive(Debug, Deserialize)]
pub struct InvokeBody<T> {
    pub ctx: InvokeBodyCtx,
    pub event: T,
    pub events: Vec<T>,
    pub steps: HashMap<String, Value>,
    pub use_api: bool,
}

#[derive(Debug, Deserialize)]
pub struct InvokeBodyCtx {
    pub attempt: u8,
    pub env: String,
    pub fn_id: String,
    pub run_id: String,
    pub stack: InvokeBodyCtxStack,
    pub step_id: String,
}

#[derive(Debug, Deserialize)]
pub struct InvokeBodyCtxStack {
    pub current: u16,
    pub stack: Vec<String>,
}

pub mod axum;

use crate::function::ServableFunction;
use serde::Deserialize;
use std::default::Default;

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

    pub fn register_fn(&mut self, func: Box<dyn ServableFunction + Sync + Send>) {
        self.funcs.push(func);
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

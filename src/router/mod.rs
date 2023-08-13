pub mod axum;

use crate::function::ServableFunction;
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

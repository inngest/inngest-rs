pub mod axum;

use crate::function::ServableFunction;
use std::default::Default;

pub struct Handler<T> {
    app_name: String,
    funcs: Vec<ServableFunction<T>>,
}

impl<T> Handler<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_name(&mut self, name: &str) {
        self.app_name = name.to_string()
    }

    pub fn register_fn(&mut self, func: &ServableFunction<T>) {
        self.funcs.push(func.clone());
    }

    pub fn register_fns(&mut self, funcs: &[ServableFunction<T>]) {
        self.funcs.extend_from_slice(funcs)
    }
}

impl<T> Default for Handler<T> {
    fn default() -> Self {
        Handler {
            app_name: String::new(),
            funcs: vec![],
        }
    }
}

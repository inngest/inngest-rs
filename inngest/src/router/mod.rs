pub mod axum;

use crate::function::ServableFn;
use serde::{Deserialize, Serialize};

pub struct Handler<T>
where
    T: Serialize + for<'a> Deserialize<'a> + 'static,
{
    app_name: String,
    funcs: Vec<ServableFn<T>>,
}

impl<T> Handler<T>
where
    T: Serialize + for<'a> Deserialize<'a> + 'static,
{
    pub fn new() -> Self {
        Handler {
            app_name: "RustDev".to_string(),
            funcs: vec![],
        }
    }

    pub fn set_name(&mut self, name: &str) {
        self.app_name = name.to_string()
    }

    pub fn register_fn(&mut self, func: ServableFn<T>) {
        self.funcs.push(func);
    }

    // pub fn register_fns(&mut self, funcs: &[ServableFunction]) {
    //     self.funcs.extend_from_slice(funcs)
    // }
}

#[derive(Deserialize)]
pub struct InvokeQuery {
    #[serde(rename = "fnId")]
    fn_id: String,
    // step: String,
}

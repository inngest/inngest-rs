pub mod axum;

use std::collections::HashMap;

use crate::{
    event::InngestEvent, function::{Function, Input, InputCtx, ServableFn, Step, StepRetry, StepRuntime}, result::Error, sdk::Request
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct Handler<T: InngestEvent> {
    app_name: String,
    funcs: Vec<ServableFn<T>>,
}

impl<T: InngestEvent> Handler<T>
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

    // sync the registered functions
    pub async fn sync(&self, framwork: &str) -> Result<(), String> {
        let functions: Vec<Function> = self
            .funcs
            .iter()
            .map(|f| {
                let mut steps = HashMap::new();
                steps.insert(
                    "step".to_string(),
                    Step {
                        id: "step".to_string(),
                        name: "step".to_string(),
                        runtime: StepRuntime {
                            url: "http://127.0.0.1:3000/api/inngest?fnId=dummy-func&step=step"
                                .to_string(),
                            method: "http".to_string(),
                        },
                        retries: StepRetry { attempts: 3 },
                    },
                );

                Function {
                    id: f.slug(),
                    name: f.slug(),
                    triggers: vec![f.trigger()],
                    steps,
                }
            })
            .collect();

        let req = Request {
            framework: framwork.to_string(),
            functions,
            url: "http://127.0.0.1:3000/api/inngest".to_string(),
            ..Default::default()
        };

        reqwest::Client::new()
            .post("http://127.0.0.1:8288/fn/register")
            .json(&req)
            .send()
            .await
            .map(|_res| ())
            .map_err(|_err| "error registering".to_string())
    }

    // run the specified function
    pub fn run(&self, query: RunQueryParams, body: &Value) -> Result<(), Error> {
        match self.funcs.iter().find(|f| f.slug() == query.fn_id) {
            None => Err(Error::Basic(format!("no function registered as ID: {}", query.fn_id))),
            Some(func) => {
                println!("Slug: {}", func.slug());
                println!("Trigger: {:?}", func.trigger());
                println!("Event: {:?}", func.event(&body["event"]));

                match func.event(&body["event"]) {
                    None => Err(Error::Basic("failed to parse event".to_string())),
                    Some(evt) => (func.func)(Input {
                        event: evt,
                        events: vec![],
                        ctx: InputCtx {
                            fn_id: String::new(),
                            run_id: String::new(),
                            step_id: String::new(),
                        },
                    })
                    .map(|_res| ()),
                }
            }
        }
    }
}

#[derive(Deserialize)]
pub struct RunQueryParams {
    #[serde(rename = "fnId")]
    fn_id: String,
}

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::{
    event::InngestEvent,
    function::{Function, Input, InputCtx, ServableFn, Step, StepRetry, StepRuntime},
    result::Error,
    sdk::Request,
    Inngest,
};

pub struct Handler<T: InngestEvent> {
    inngest: Inngest,
    funcs: HashMap<String, ServableFn<T>>,
}

#[derive(Deserialize)]
pub struct RunQueryParams {
    #[serde(rename = "fnId")]
    fn_id: String,
}

impl<T: InngestEvent> Handler<T> {
    pub fn new(client: Inngest) -> Self {
        Handler {
            inngest: client.clone(),
            funcs: HashMap::new(),
        }
    }

    pub fn register_fn(&mut self, func: ServableFn<T>) {
        let fn_id = func.opts.id.clone();
        self.funcs.insert(fn_id, func);
    }

    pub async fn sync(&self, framework: &str) -> Result<(), String> {
        let functions: Vec<Function> = self
            .funcs
            .iter()
            .map(|(_, f)| {
                let mut steps = HashMap::new();
                steps.insert(
                    "step".to_string(),
                    Step {
                        id: "step".to_string(),
                        name: "step".to_string(),
                        runtime: StepRuntime {
                            url: format!(
                                "http://127.0.0.1:3000/api/inngest?fnId={}&step=step",
                                f.slug()
                            ),
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
            framework: framework.to_string(),
            functions,
            url: "http://127.0.0.1:3000/api/inngest".to_string(),
            ..Default::default()
        };

        reqwest::Client::new()
            .post("http://127.0.0.1:8288/fn/register")
            .json(&req)
            .send()
            .await
            .map(|_| ())
            .map_err(|_err| "error registering".to_string())
    }

    pub fn run(&self, query: RunQueryParams, body: &Value) -> Result<Value, Error> {
        match self.funcs.get(&query.fn_id) {
            None => Err(Error::Basic(format!(
                "no function registered as ID: {}",
                &query.fn_id
            ))),
            Some(func) => match func.event(&body["event"]) {
                None => Err(Error::Basic("failed to parse event".to_string())),
                Some(evt) => (func.func)(Input {
                    event: evt,
                    events: vec![],
                    ctx: InputCtx {
                        fn_id: query.fn_id.clone(),
                        run_id: String::new(),
                        step_id: String::new(),
                    },
                }),
            },
        }
    }
}

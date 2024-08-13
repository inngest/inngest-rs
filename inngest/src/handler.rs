use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::{
    config::Config,
    event::InngestEvent,
    function::{Function, Input, InputCtx, ServableFn, Step, StepRetry, StepRuntime},
    result::{Error, SdkResponse},
    sdk::Request,
    Inngest,
};

pub struct Handler<T: InngestEvent> {
    inngest: Inngest,
    signing_key: Option<String>,
    // TODO: signing_key_fallback
    serve_origin: Option<String>,
    serve_path: Option<String>,
    funcs: HashMap<String, ServableFn<T>>,
}

#[derive(Deserialize)]
pub struct RunQueryParams {
    #[serde(rename = "fnId")]
    fn_id: String,
}

impl<T: InngestEvent> Handler<T> {
    pub fn new(client: Inngest) -> Self {
        let signing_key = Config::signing_key();
        let serve_origin = Config::serve_origin();
        let serve_path = Config::serve_path();

        Handler {
            signing_key,
            serve_origin,
            serve_path,
            inngest: client.clone(),
            funcs: HashMap::new(),
        }
    }

    pub fn register_fn(&mut self, func: ServableFn<T>) {
        self.funcs.insert(func.slug(), func);
    }

    pub async fn sync(&self, _headers: &HashMap<String, String>, framework: &str) -> Result<(), String> {
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
            app_name: self.inngest.app_id.clone(),
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

    pub fn run(&self, query: RunQueryParams, body: &Value) -> Result<SdkResponse, Error> {
        match self.funcs.get(&query.fn_id) {
            None => Err(Error::Basic(format!(
                "no function registered as ID: {}",
                &query.fn_id
            ))),
            Some(func) => match func.event(&body["event"]) {
                None => Err(Error::Basic("failed to parse event".to_string())),
                Some(evt) => {
                    let res = (func.func)(Input {
                        event: evt,
                        events: vec![],
                        ctx: InputCtx {
                            fn_id: query.fn_id.clone(),
                            run_id: String::new(),
                            step_id: String::new(),
                        },
                    });

                    res.map(|v| SdkResponse {
                        status: 200,
                        body: v,
                    })
                }
            },
        }
    }
}

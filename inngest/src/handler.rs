use std::collections::HashMap;

use inngest_macros::InngestEvent;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    config::Config,
    event::{Event, InngestEvent},
    function::{Function, Input, InputCtx, ServableFn, Step, StepRetry, StepRuntime},
    result::{Error, SdkResponse},
    sdk::Request,
    step_tool::Step as StepTool,
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

    pub async fn sync(
        &self,
        _headers: &HashMap<String, String>,
        framework: &str,
    ) -> Result<(), String> {
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
                                // TODO: fix the URL
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
            // TODO: fix the URL
            url: "http://127.0.0.1:3000/api/inngest".to_string(),
            ..Default::default()
        };

        reqwest::Client::new()
            // TODO: fix the URL
            .post("http://127.0.0.1:8288/fn/register")
            .json(&req)
            .send()
            .await
            .map(|_| ())
            .map_err(|_err| "error registering".to_string())
    }

    pub fn run(&self, query: RunQueryParams, body: &Value) -> Result<SdkResponse, Error> {
        let data = match serde_json::from_value::<RunRequestBody<T>>(body.clone()) {
            Ok(res) => res,
            Err(err) => {
                // TODO: need to surface this error better
                let msg = Error::Basic(format!("error parsing run request: {}", err));
                return Err(msg);
            }
        };

        // TODO: retrieve data from API on flag
        if data.use_api {}

        // find the specified function
        let Some(func) = self.funcs.get(&query.fn_id) else {
            return Err(Error::Basic(format!(
                "no function registered as ID: {}",
                &query.fn_id
            )));
        };

        let input = Input {
            event: data.event,
            events: data.events,
            ctx: InputCtx {
                fn_id: query.fn_id.clone(),
                run_id: data.ctx.run_id.clone(),
                step_id: "step".to_string(),
            },
        };

        let mut step_tool = StepTool::new(&data.steps);

        // run the function
        match (func.func)(&input, &mut step_tool) {
            Ok(v) => Ok(SdkResponse {
                status: 200,
                body: v,
            }),

            Err(err) => match err {
                Error::StepGenerator => {
                    let body = match serde_json::to_value(&step_tool.genop) {
                        Ok(v) => v,
                        Err(err) => {
                            return Err(Error::Basic(format!(
                                "error serializing step response: {}",
                                err
                            )));
                        }
                    };

                    println!("RESP: {:#?}", body);

                    Ok(SdkResponse { status: 206, body })
                }
                _ => Err(err),
            },
        }
    }
}

#[derive(Deserialize)]
struct RunRequestBody<T: 'static> {
    ctx: RunRequestCtx,
    event: Event<T>,
    events: Vec<Event<T>>,
    use_api: bool,
    steps: HashMap<String, Option<String>>,
    version: i32,
}

#[derive(Deserialize)]
struct RunRequestCtx {
    attempt: u8,
    disable_immediate_execution: bool,
    env: String,
    fn_id: String,
    run_id: String,
    step_id: String,
    stack: RunRequestCtxStack,
}

#[derive(Deserialize)]
struct RunRequestCtxStack {
    current: u32,
    stack: Vec<String>,
}

use std::{collections::HashMap, fmt::Debug, panic::AssertUnwindSafe};

use futures::FutureExt;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    basic_error,
    client::Inngest,
    config::Config,
    event::Event,
    function::{Function, Input, InputCtx, ServableFn, Step, StepRetry, StepRuntime},
    result::{Error, FlowControlVariant, SdkResponse},
    sdk::Request,
    step_tool::Step as StepTool,
    header
};

pub struct Handler<T: 'static, E> {
    inngest: Inngest,
    signing_key: Option<String>,
    // TODO: signing_key_fallback
    serve_origin: Option<String>,
    serve_path: Option<String>,
    funcs: HashMap<String, ServableFn<T, E>>,
}

#[derive(Deserialize)]
pub struct RunQueryParams {
    #[serde(rename = "fnId")]
    fn_id: String,
}

impl<T, E> Handler<T, E> {
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

    pub fn signing_key(mut self, key: &str) -> Self {
        self.signing_key = Some(key.to_string());
        self
    }

    pub fn serve_origin(mut self, origin: &str) -> Self {
        self.serve_origin = Some(origin.to_string());
        self
    }

    pub fn serve_path(mut self, path: &str) -> Self {
        self.serve_path = Some(path.to_string());
        self
    }

    pub fn register_fn(&mut self, func: ServableFn<T, E>) {
        self.funcs.insert(func.slug(), func);
    }

    fn app_serve_origin(&self, _headers: &HashMap<String, String>) -> String {
        if let Some(origin) = self.serve_origin.clone() {
            return origin;
        }
        // if let Some(host) = headers.get("host") {
        //     return host.to_string();
        // }

        "http://127.0.0.1:3000".to_string()
    }

    fn app_serve_path(&self) -> String {
        if let Some(path) = self.serve_path.clone() {
            return path;
        }
        "/api/inngest".to_string()
    }

    pub async fn sync(
        &self,
        headers: &HashMap<String, String>,
        framework: &str,
    ) -> Result<(), String> {
        let kind = match headers.get(header::INNGEST_SERVER_KIND) {
            Some(val) => match val.as_str() {
                "cloud" => Kind::Cloud,
                _ => Kind::Dev
            },
            None => Kind::Dev
        };

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
                                "{}{}?fnId={}&step=step",
                                self.app_serve_origin(headers),
                                self.app_serve_path(),
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
            url: format!("{}{}", self.app_serve_origin(headers), self.app_serve_path()),
            ..Default::default()
        };

        reqwest::Client::new()
            .post(format!("{}/fn/register", self.inngest.inngest_api_origin(kind)))
            .json(&req)
            .send()
            .await
            .map(|_| ())
            .map_err(|_err| "error registering".to_string())
    }

    pub async fn run(&self, query: RunQueryParams, body: &Value) -> Result<SdkResponse, Error>
    where
        T: for<'de> Deserialize<'de> + Debug,
        E: Into<Error>,
    {
        let data = match serde_json::from_value::<RunRequestBody<T>>(body.clone()) {
            Ok(res) => res,
            Err(err) => {
                // TODO: need to surface this error better
                let msg = basic_error!("error parsing run request: {}", err);
                return Err(msg);
            }
        };

        // TODO: retrieve data from API on flag
        if data.use_api {}

        // find the specified function
        let Some(func) = self.funcs.get(&query.fn_id) else {
            return Err(basic_error!(
                "no function registered as ID: {}",
                &query.fn_id
            ));
        };

        let input = Input {
            event: data.event,
            events: data.events,
            ctx: InputCtx {
                env: data.ctx.env.clone(),
                fn_id: query.fn_id.clone(),
                run_id: data.ctx.run_id.clone(),
                step_id: "step".to_string(),
                attempt: data.ctx.attempt,
            },
        };

        let step_tool = StepTool::new(&self.inngest.app_id, &data.steps);

        match std::panic::catch_unwind(AssertUnwindSafe(|| (func.func)(input, step_tool.clone()))) {
            Ok(fut) => {
                match AssertUnwindSafe(fut).catch_unwind().await {
                    Ok(v) => match v {
                        Ok(v) => Ok(SdkResponse {
                            status: 200,
                            body: v,
                        }),
                        Err(err) => match err.into() {
                            Error::Interrupt(mut flow) => {
                                flow.acknowledge();
                                match flow.variant {
                                    FlowControlVariant::StepGenerator => {
                                        let (status, body) = if step_tool.error().is_some() {
                                            match serde_json::to_value(&step_tool.error()) {
                                                Ok(v) => {
                                                    // TODO: check current attempts and see if it can retry or not
                                                    (500, v)
                                                }
                                                Err(err) => {
                                                    return Err(basic_error!(
                                                        "error seralizing step error: {}",
                                                        err
                                                    ));
                                                }
                                            }
                                        } else if step_tool.genop().len() > 0 {
                                            // TODO: only expecting one for now, will need to handle multiple
                                            match serde_json::to_value(&step_tool.genop()) {
                                                Ok(v) => (206, v),
                                                Err(err) => {
                                                    return Err(basic_error!(
                                                        "error serializing step response: {}",
                                                        err
                                                    ));
                                                }
                                            }
                                        } else {
                                            (206, json!("null"))
                                        };
                                        Ok(SdkResponse { status, body })
                                    }
                                }
                            }
                            other => Err(other),
                        },
                    },
                    Err(panic_err) => Ok(SdkResponse {
                        status: 500,
                        body: Value::String(format!("panic: {:?}", panic_err)),
                    }),
                }
            }
            Err(panic_err) => Ok(SdkResponse {
                status: 500,
                body: Value::String(format!("panic: {:?}", panic_err)),
            }),
        }
    }
    // run the function
}

#[derive(Deserialize, Debug)]
struct RunRequestBody<T: 'static> {
    ctx: RunRequestCtx,
    event: Event<T>,
    events: Vec<Event<T>>,
    use_api: bool,
    steps: HashMap<String, Option<Value>>,
    version: i32,
}

#[derive(Deserialize, Debug)]
struct RunRequestCtx {
    attempt: u8,
    disable_immediate_execution: bool,
    env: String,
    fn_id: String,
    run_id: String,
    step_id: String,
    stack: RunRequestCtxStack,
}

#[derive(Deserialize, Debug)]
struct RunRequestCtxStack {
    current: u32,
    stack: Vec<String>,
}

#[derive(Clone)]
pub(crate) enum Kind {
    Dev,
    Cloud
}

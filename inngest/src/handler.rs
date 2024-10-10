use std::{collections::HashMap, fmt::Debug, panic::AssertUnwindSafe};

use futures::FutureExt;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    basic_error,
    client::Inngest,
    config::Config,
    event::Event,
    function::{Function, Input, InputCtx, ServableFn},
    header::Headers,
    result::{Error, FlowControlVariant, SdkResponse},
    sdk::Request,
    signature::Signature,
    step_tool::Step as StepTool,
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
    pub fn new(client: &Inngest) -> Self {
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

    pub fn register_fns(&mut self, funcs: Vec<ServableFn<T, E>>) {
        for f in funcs {
            self.funcs.insert(f.slug(), f);
        }
    }

    fn app_serve_origin(&self, headers: &Headers) -> String {
        if let Some(origin) = self.serve_origin.clone() {
            return origin;
        }
        println!("HEADERS {:#?}", headers);

        if let Some(host) = headers.host() {
            return host;
        }

        "http://127.0.0.1:3000".to_string()
    }

    fn app_serve_path(&self) -> String {
        if let Some(path) = self.serve_path.clone() {
            return path;
        }
        "/api/inngest".to_string()
    }

    pub async fn sync(&self, headers: &Headers, framework: &str) -> Result<(), String> {
        let kind = headers.server_kind();

        let app_id = self.inngest.app_id();
        let functions: Vec<Function> = self
            .funcs
            .iter()
            .map(|(_, f)| f.function(&self.app_serve_origin(headers), &self.app_serve_path()))
            .collect();

        let req = Request {
            app_name: app_id.clone(),
            framework: framework.to_string(),
            functions,
            url: format!(
                "{}{}",
                self.app_serve_origin(headers),
                self.app_serve_path()
            ),
            ..Default::default()
        };

        let mut req = reqwest::Client::new()
            .post(format!(
                "{}/fn/register",
                self.inngest.inngest_api_origin(kind)
            ))
            .json(&req);

        if let Some(key) = &self.signing_key {
            let sig = Signature::new(&key);
            match sig.hash() {
                Ok(hashed) => {
                    req = req.header("authorization", format!("Bearer {}", hashed));
                }
                Err(_err) => {
                    return Err("error hashing signing key".to_string());
                }
            }
        }

        req.send()
            .await
            .map(|_| ())
            .map_err(|_err| "error registering".to_string())
    }

    pub async fn run(
        &self,
        headers: &Headers,
        query: RunQueryParams,
        raw_body: &str,
        body: &Value,
    ) -> Result<SdkResponse, Error>
    where
        T: for<'de> Deserialize<'de> + Debug,
        E: Into<Error>,
    {
        let sig = headers.signature();
        let kind = headers.server_kind();
        if kind == Kind::Cloud && sig.is_none() {
            return Err(basic_error!("no signature provided for SDK in Cloud mode"));
        }

        // Verify the signature if provided
        if let Some(sig) = sig.clone() {
            let Some(key) = self.signing_key.clone() else {
                return Err(basic_error!(
                    "no signing key available for verifying request signature"
                ));
            };

            let signature = Signature::new(&key).sig(&sig).body(raw_body);
            _ = signature.verify(false)?;
        }

        let data = match serde_json::from_value::<RunRequestBody<T>>(body.clone()) {
            Ok(res) => res,
            Err(err) => {
                println!("ERROR: {:?}", err);
                println!("BODY: {:#?}", &body);
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

        let step_tool = StepTool::new(&data.steps);

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
    // version: i32,
}

#[derive(Deserialize, Debug)]
struct RunRequestCtx {
    attempt: u8,
    // disable_immediate_execution: bool,
    env: String,
    // fn_id: String,
    run_id: String,
    // step_id: String,
    // stack: RunRequestCtxStack,
}

#[derive(Deserialize, Debug)]
struct RunRequestCtxStack {
    // current: u32,
    // stack: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Kind {
    Dev,
    Cloud,
}

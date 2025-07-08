use std::{collections::HashMap, fmt::Debug, panic::AssertUnwindSafe};

use futures::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::Digest;
use sha2::Sha256;

use crate::{
    basic_error,
    client::{self, Inngest},
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
    mode: Kind,
}

#[derive(Deserialize)]
pub struct RunQueryParams {
    #[serde(rename = "fnId")]
    fn_id: String,
}

#[derive(Deserialize, Debug)]
pub struct SyncQueryParams {
    #[serde(rename = "deployId")]
    deploy_id: Option<String>,
}

impl<T, E> Handler<T, E> {
    pub fn new(client: &Inngest) -> Self {
        let signing_key = Config::signing_key();
        let serve_origin = Config::serve_origin();
        let serve_path = Config::serve_path();
        let mode = match client.dev.clone() {
            None => Kind::Cloud,
            Some(v) => {
                if v != "0" {
                    Kind::Dev
                } else {
                    Kind::Cloud
                }
            }
        };

        Handler {
            signing_key,
            serve_origin,
            serve_path,
            inngest: client.clone(),
            funcs: HashMap::new(),
            mode,
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

        if let Some(host) = headers.host() {
            if host.contains("localhost") {
                return format!("http://{}", &host);
            }

            // TODO:
            // - check scheme header
            // - check the url include 127.0.0.1 or localhost
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

    fn verify_signature(&self, sig: &str, raw_body: &str) -> Result<(), Error> {
        let Some(key) = self.signing_key.clone() else {
            return Err(basic_error!(
                "no signing key available for verifying request signature"
            ));
        };

        let signature = Signature::new(&key).sig(&sig).body(raw_body);
        signature.verify(false)
    }

    pub async fn introspect(
        &self,
        headers: &Headers,
        framework: &str,
        raw_body: &str,
    ) -> Result<IntrospectResult, Error> {
        let payload = self.sync_payload(headers, framework);
        let function_count = payload.functions.len() as u32;
        let has_event_key = self.has_event_key();
        let has_signing_key = self.has_signing_key();
        let has_signing_key_fallback = self.has_signing_key_fallback();
        let schema_version = PROBE_SCHEMA_VERSION.to_string();

        let mut authentication_succeeded: Option<bool> = None;

        match self.mode {
            Kind::Dev => Ok(IntrospectResult::Unauthenticated(
                IntrospectUnauthedResult {
                    authentication_succeeded,
                    extra: None,
                    function_count,
                    has_event_key,
                    has_signing_key,
                    has_signing_key_fallback,
                    mode: Kind::Dev,
                    schema_version,
                },
            )),

            Kind::Cloud => {
                if let Some(sig) = headers.signature() {
                    if let Ok(_) = self.verify_signature(&sig, raw_body) {
                        let api_origin = match self.inngest.api_origin.clone() {
                            Some(origin) => origin,
                            None => client::API_ORIGIN.to_string(),
                        };

                        let event_api_origin = match self.inngest.event_api_origin.clone() {
                            Some(origin) => origin,
                            None => client::EVENT_API_ORIGIN.to_string(),
                        };

                        let event_key_hash = self.hash_key(self.inngest.event_key.clone());
                        let signing_key_hash = if let Some(key) = self.signing_key.clone() {
                            match Signature::new(&key).hash() {
                                Ok(hash) => Some(hash),
                                Err(_) => None,
                            }
                        } else {
                            None
                        };

                        return Ok(IntrospectResult::Authenticated(IntrospectAuthedResult {
                            app_id: self.inngest.app_id(),
                            api_origin,
                            event_api_origin,
                            event_key_hash,
                            authentication_succeeded: true,
                            env: None, // TODO
                            extra: None,
                            framework: framework.to_string(),
                            function_count,
                            has_event_key,
                            has_signing_key,
                            has_signing_key_fallback,
                            mode: Kind::Cloud,
                            schema_version,
                            sdk_language: "rust".to_string(),
                            sdk_version: env!("CARGO_PKG_VERSION").to_string(),
                            serve_origin: self.serve_origin.clone(),
                            serve_path: self.serve_path.clone(),
                            signing_key_hash,
                            signing_key_fallback_hash: None, // TODO
                        }));
                    };
                    // mark it as false
                    authentication_succeeded = Some(false);
                }

                Ok(IntrospectResult::Unauthenticated(
                    IntrospectUnauthedResult {
                        authentication_succeeded,
                        extra: None,
                        function_count,
                        has_event_key,
                        has_signing_key,
                        has_signing_key_fallback,
                        mode: Kind::Cloud,
                        schema_version,
                    },
                ))
            }
        }
    }

    fn has_event_key(&self) -> bool {
        self.inngest.event_key.is_some()
    }

    fn has_signing_key(&self) -> bool {
        self.signing_key.is_some()
    }

    fn has_signing_key_fallback(&self) -> bool {
        false
    }

    fn hash_key(&self, key: Option<String>) -> Option<String> {
        key.map(|key| {
            let mut hasher = Sha256::new();
            hasher.update(key.as_bytes());

            let sum = hasher.finalize();
            base16::encode_lower(&sum)
        })
    }

    fn sync_payload(&self, headers: &Headers, framework: &str) -> Request {
        let app_id = self.inngest.app_id();
        let functions: Vec<Function> = self
            .funcs
            .iter()
            .map(|(_, f)| f.function(&self.app_serve_origin(headers), &self.app_serve_path()))
            .collect();

        Request {
            app_name: app_id.clone(),
            framework: framework.to_string(),
            functions,
            url: format!(
                "{}{}",
                self.app_serve_origin(headers),
                self.app_serve_path()
            ),
            ..Default::default()
        }
    }

    pub async fn sync(
        &self,
        headers: &Headers,
        query: &SyncQueryParams,
        framework: &str,
    ) -> Result<SyncResponse, String> {
        let kind = headers.server_kind();
        let req = self.sync_payload(headers, framework);

        let mut sync_req = reqwest::Client::new()
            .post(format!(
                "{}/fn/register",
                self.inngest.inngest_api_origin(kind)
            ))
            .json(&req);

        if let Some(key) = &self.signing_key {
            let sig = Signature::new(&key);
            match sig.hash() {
                Ok(hashed) => {
                    sync_req = sync_req.header("authorization", format!("Bearer {}", hashed));
                }
                Err(_err) => {
                    return Err("error hashing signing key".to_string());
                }
            }
        }

        if let Some(deploy_id) = query.deploy_id.clone() {
            sync_req = sync_req.query(&[("deployId", &deploy_id)]);
        }

        match sync_req.send().await {
            Ok(resp) => match resp.json::<InngestSyncSuccess>().await {
                Ok(res) => {
                    let modified = match res.modified.clone() {
                        None => false,
                        Some(v) => v,
                    };

                    Ok(SyncResponse::OutOfBand(OutOfBandSyncResponse {
                        message: "Successfully synced.".to_string(),
                        modified,
                    }))
                }
                Err(_) => {
                    return Err("error parsing sync response".to_string());
                }
            },
            Err(err) => {
                println!("ERROR: {:?}", err);

                Err("error registering".to_string())
            }
        }
    }

    pub async fn run(
        &self,
        headers: &Headers,
        query: &RunQueryParams,
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
            _ = self.verify_signature(&sig, raw_body)?;
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

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Dev,
    Cloud,
}

const PROBE_SCHEMA_VERSION: &str = "2024-05-24";

#[derive(Serialize)]
#[serde(untagged)]
pub enum IntrospectResult {
    Unauthenticated(IntrospectUnauthedResult),
    Authenticated(IntrospectAuthedResult),
}

#[derive(Serialize)]
pub struct IntrospectUnauthedResult {
    authentication_succeeded: Option<bool>,
    extra: Option<Value>,
    function_count: u32,
    has_event_key: bool,
    has_signing_key: bool,
    has_signing_key_fallback: bool,
    mode: Kind,
    schema_version: String,
}

#[derive(Serialize)]
pub struct IntrospectAuthedResult {
    app_id: String,
    api_origin: String,
    event_api_origin: String,
    event_key_hash: Option<String>,
    authentication_succeeded: bool,
    env: Option<String>,
    extra: Option<Value>,
    framework: String,
    function_count: u32,
    has_event_key: bool,
    has_signing_key: bool,
    has_signing_key_fallback: bool,
    mode: Kind,
    schema_version: String,
    sdk_language: String,
    sdk_version: String,
    serve_origin: Option<String>,
    serve_path: Option<String>,
    signing_key_fallback_hash: Option<String>,
    signing_key_hash: Option<String>,
}

#[derive(Serialize)]
pub enum SyncResponse {
    InBand(InBandSyncResponse),
    OutOfBand(OutOfBandSyncResponse),
}

#[derive(Serialize)]
pub struct InBandSyncResponse {
    app_id: String,
    framework: String,
    functions: Vec<Function>,
    inspection: IntrospectAuthedResult,
    sdk_author: String,
    sdk_language: String,
    sdk_version: String,
    url: String,
}

#[derive(Serialize)]
pub struct OutOfBandSyncResponse {
    message: String,
    modified: bool,
}

#[derive(Deserialize)]
pub struct InngestSyncError {
    error: String,
}

#[derive(Deserialize, Debug)]
pub struct InngestSyncSuccess {
    ok: bool,
    modified: Option<bool>,
}

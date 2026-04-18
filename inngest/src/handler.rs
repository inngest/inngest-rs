use std::{collections::HashMap, panic::AssertUnwindSafe, sync::Arc};

use futures::{future::BoxFuture, FutureExt};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::Digest;
use sha2::Sha256;
use slug::slugify;

use crate::{
    basic_error,
    client::{self, Inngest},
    config::Config,
    event::{Event, InngestEvent},
    function::{Function, FunctionOpts, Input, InputCtx, ServableFn, Trigger},
    header::{self, Headers},
    result::{Error, FlowControlVariant, SdkResponse},
    sdk::Request,
    signature::Signature,
    step_tool::Step as StepTool,
    version::{self, EXECUTION_VERSION},
};

type DynamicFn = dyn Fn(RunQueryParams, Value) -> BoxFuture<'static, Result<SdkResponse, Error>>
    + Send
    + Sync
    + 'static;

struct DynamicServableFn {
    app_id: String,
    opts: FunctionOpts,
    trigger: Trigger,
    func: Box<DynamicFn>,
}

impl DynamicServableFn {
    fn slug(&self) -> String {
        format!("{}-{}", &self.app_id, slugify(self.opts.id.clone()))
    }

    fn function(&self, serve_origin: &str, serve_path: &str) -> Function {
        let id = self.slug();
        let name = match self.opts.name.clone() {
            Some(name) => name,
            None => id.clone(),
        };

        let mut steps = HashMap::new();
        steps.insert(
            "step".to_string(),
            crate::function::Step {
                id: "step".to_string(),
                name: "step".to_string(),
                runtime: crate::function::StepRuntime {
                    url: format!("{}{}?fnId={}&stepId=step", serve_origin, serve_path, &id),
                    method: "http".to_string(),
                },
                retries: crate::function::StepRetry {
                    attempts: self.opts.retries,
                },
            },
        );

        Function {
            id,
            name,
            triggers: vec![self.trigger.clone()],
            steps,
        }
    }

    async fn run(&self, query: RunQueryParams, body: Value) -> Result<SdkResponse, Error> {
        (self.func)(query, body).await
    }
}

/// A type-erased function registration used by [`Handler::register_fns`].
///
/// Convert a [`ServableFn`] into a `RegisteredFn` with `.into()` when batching
/// functions that use different event payload or error types.
pub struct RegisteredFn(DynamicServableFn);

impl RegisteredFn {
    fn into_dynamic(self) -> DynamicServableFn {
        self.0
    }
}

impl<T, E> From<ServableFn<T, E>> for DynamicServableFn
where
    T: InngestEvent + Send,
    E: Into<Error> + 'static,
{
    fn from(func: ServableFn<T, E>) -> Self {
        let ServableFn {
            app_id,
            client,
            opts,
            trigger,
            func,
        } = func;
        let func = Arc::new(func);

        Self {
            app_id,
            opts,
            trigger,
            func: Box::new(move |query, body| {
                let step_func = Arc::clone(&func);
                let client = client.clone();

                async move {
                    let data = match serde_json::from_value::<RunRequestBody<T>>(body.clone()) {
                        Ok(res) => res,
                        Err(err) => {
                            println!("ERROR: {:?}", err);
                            println!("BODY: {:#?}", &body);
                            let msg = basic_error!("error parsing run request: {}", err);
                            return Err(msg);
                        }
                    };

                    let _ = data._use_api;

                    let input = Input {
                        event: data.event,
                        events: data.events,
                        ctx: InputCtx {
                            env: data.ctx.env.clone(),
                            fn_id: query.fn_id.clone(),
                            run_id: data.ctx.run_id.clone(),
                            step_id: query.step_id.clone(),
                            attempt: data.ctx.attempt,
                        },
                    };

                    let step_tool = StepTool::new(
                        client.clone(),
                        &data.steps,
                        &query.step_id,
                        &data.ctx.stack.stack,
                    );

                    match std::panic::catch_unwind(AssertUnwindSafe(|| {
                        (step_func.as_ref())(input, step_tool.clone())
                    })) {
                        Ok(fut) => match AssertUnwindSafe(fut).catch_unwind().await {
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
                                                let (status, body) = if let Some(step_id) = step_tool.missing_step()
                                                {
                                                    (
                                                        206,
                                                        json!([{
                                                            "id": step_id,
                                                            "op": "StepNotFound"
                                                        }]),
                                                    )
                                                } else if !step_tool.genop().is_empty() {
                                                    match serde_json::to_value(step_tool.genop()) {
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
                        },
                        Err(panic_err) => Ok(SdkResponse {
                            status: 500,
                            body: Value::String(format!("panic: {:?}", panic_err)),
                        }),
                    }
                }
                .boxed()
            }),
        }
    }
}

impl<T, E> From<ServableFn<T, E>> for RegisteredFn
where
    T: InngestEvent + Send,
    E: Into<Error> + 'static,
{
    fn from(func: ServableFn<T, E>) -> Self {
        Self(DynamicServableFn::from(func))
    }
}

/// An Inngest app handler that serves registered functions over HTTP.
///
/// A single handler can register functions with different event payload types.
/// Use [`Handler::register_fn`] for one function at a time, or
/// [`Handler::register_fns`] together with [`RegisteredFn`] to batch mixed
/// function types.
pub struct Handler {
    inngest: Inngest,
    signing_key: Option<String>,
    signing_key_fallback: Option<String>,
    serve_origin: Option<String>,
    serve_path: Option<String>,
    funcs: HashMap<String, DynamicServableFn>,
    mode: Kind,
}

#[derive(Clone, Deserialize)]
pub struct RunQueryParams {
    #[serde(rename = "fnId")]
    fn_id: String,
    #[serde(default = "default_step_id", rename = "stepId")]
    step_id: String,
}

#[derive(Deserialize, Debug)]
pub struct SyncQueryParams {
    #[serde(rename = "deployId")]
    deploy_id: Option<String>,
}

fn default_step_id() -> String {
    "step".to_string()
}

impl Handler {
    /// Creates a new handler for the given Inngest client.
    pub fn new(client: &Inngest) -> Self {
        let signing_key = Config::signing_key();
        let signing_key_fallback = Config::signing_key_fallback();
        let serve_origin = Config::serve_origin();
        let serve_path = Config::serve_path();
        let mode = client.mode();

        Handler {
            signing_key,
            signing_key_fallback,
            serve_origin,
            serve_path,
            inngest: client.clone(),
            funcs: HashMap::new(),
            mode,
        }
    }

    /// Overrides the signing key used to verify inbound requests.
    pub fn signing_key(mut self, key: &str) -> Self {
        self.signing_key = Some(key.to_string());
        self
    }

    /// Overrides the fallback signing key used after primary-key auth fails.
    pub fn signing_key_fallback(mut self, key: &str) -> Self {
        self.signing_key_fallback = Some(key.to_string());
        self
    }

    /// Overrides the public origin used when syncing function URLs.
    pub fn serve_origin(mut self, origin: &str) -> Self {
        self.serve_origin = Some(origin.to_string());
        self
    }

    /// Overrides the public path used when syncing function URLs.
    pub fn serve_path(mut self, path: &str) -> Self {
        self.serve_path = Some(path.to_string());
        self
    }

    /// Registers a single function with the handler.
    ///
    /// This is the simplest option when adding one function at a time,
    /// regardless of its event payload type.
    pub fn register_fn<T, E>(&mut self, func: ServableFn<T, E>)
    where
        T: InngestEvent + Send,
        E: Into<Error> + 'static,
    {
        let func = DynamicServableFn::from(func);
        self.funcs.insert(func.slug(), func);
    }

    /// Registers multiple functions with the handler.
    ///
    /// This accepts any iterator of [`RegisteredFn`], which allows batching
    /// heterogeneous functions:
    ///
    /// ```ignore
    /// handler.register_fns(vec![
    ///     hello_fn(&client).into(),
    ///     step_run_fn(&client).into(),
    /// ]);
    /// ```
    pub fn register_fns<I>(&mut self, funcs: I)
    where
        I: IntoIterator<Item = RegisteredFn>,
    {
        for f in funcs {
            let func = f.into_dynamic();
            self.funcs.insert(func.slug(), func);
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

        let signature = Signature::new(&key).sig(sig).body(raw_body);
        match signature.verify(false) {
            Ok(_) => Ok(()),
            Err(err) => {
                if let Some(fallback) = self.signing_key_fallback.clone() {
                    let signature = Signature::new(&fallback).sig(sig).body(raw_body);
                    signature.verify(false)
                } else {
                    Err(err)
                }
            }
        }
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
        let authentication_succeeded = headers
            .signature()
            .map(|sig| self.verify_signature(&sig, raw_body).is_ok());

        if authentication_succeeded == Some(true) {
            let api_origin = match self.inngest.api_origin.clone() {
                Some(origin) => origin,
                None => client::API_ORIGIN.to_string(),
            };

            let event_api_origin = match self.inngest.event_api_origin.clone() {
                Some(origin) => origin,
                None => client::EVENT_API_ORIGIN.to_string(),
            };

            let event_key_hash = self.hash_key(self.inngest.event_key.clone());
            let signing_key_hash = self.hashed_signing_key(self.signing_key.clone());
            let signing_key_fallback_hash =
                self.hashed_signing_key(self.signing_key_fallback.clone());

            return Ok(IntrospectResult::Authenticated(Box::new(
                IntrospectAuthedResult {
                    app_id: self.inngest.app_id(),
                    api_origin,
                    event_api_origin,
                    event_key_hash,
                    authentication_succeeded: true,
                    env: self.inngest.env.clone(),
                    extra: None,
                    framework: framework.to_string(),
                    function_count,
                    has_event_key,
                    has_signing_key,
                    has_signing_key_fallback,
                    mode: self.mode.clone(),
                    schema_version,
                    sdk_language: "rust".to_string(),
                    sdk_version: env!("CARGO_PKG_VERSION").to_string(),
                    serve_origin: self.serve_origin.clone(),
                    serve_path: self.serve_path.clone(),
                    signing_key_hash,
                    signing_key_fallback_hash,
                },
            )));
        }

        Ok(IntrospectResult::Unauthenticated(Box::new(
            IntrospectUnauthedResult {
                authentication_succeeded,
                extra: None,
                function_count,
                has_event_key,
                has_signing_key,
                has_signing_key_fallback,
                mode: self.mode.clone(),
                schema_version,
            },
        )))
    }

    fn has_event_key(&self) -> bool {
        self.inngest.event_key.is_some()
    }

    fn has_signing_key(&self) -> bool {
        self.signing_key.is_some()
    }

    fn has_signing_key_fallback(&self) -> bool {
        self.signing_key_fallback.is_some()
    }

    fn hash_key(&self, key: Option<String>) -> Option<String> {
        key.map(|key| {
            let mut hasher = Sha256::new();
            hasher.update(key.as_bytes());

            let sum = hasher.finalize();
            base16::encode_lower(&sum)
        })
    }

    fn hashed_signing_key(&self, key: Option<String>) -> Option<String> {
        key.and_then(|key| Signature::new(&key).hash().ok())
    }

    fn sync_payload(&self, headers: &Headers, framework: &str) -> Request {
        let app_id = self.inngest.app_id();
        let functions: Vec<Function> = self
            .funcs
            .values()
            .map(|f| f.function(&self.app_serve_origin(headers), &self.app_serve_path()))
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
        _headers: &Headers,
        query: &SyncQueryParams,
        framework: &str,
    ) -> Result<SyncResponse, String> {
        let req = self.sync_payload(_headers, framework);
        let sync_url = format!(
            "{}/fn/register",
            self.inngest.inngest_api_origin().trim_end_matches('/')
        );

        let mut resp = self
            .send_sync_request(&sync_url, &req, query, self.signing_key.as_deref())
            .await?;

        if resp.status().as_u16() == 401
            && self.signing_key.is_some()
            && self.signing_key_fallback.is_some()
        {
            resp = self
                .send_sync_request(&sync_url, &req, query, self.signing_key_fallback.as_deref())
                .await?;
        }

        let status = resp.status();
        let body = match resp.text().await {
            Ok(body) => body,
            Err(err) => {
                return Err(format!("error reading sync response: {}", err));
            }
        };

        if !status.is_success() {
            return Err(format!(
                "error registering: status {} body {}",
                status.as_u16(),
                body
            ));
        }

        match serde_json::from_str::<InngestSyncSuccess>(&body) {
            Ok(res) => {
                let modified: bool = res.modified.unwrap_or_default();

                Ok(SyncResponse::OutOfBand(Box::new(OutOfBandSyncResponse {
                    message: "Successfully synced.".to_string(),
                    modified,
                })))
            }
            Err(err) => Err(format!(
                "error parsing sync response: {} body {}",
                err, body
            )),
        }
    }

    pub async fn run(
        &self,
        headers: &Headers,
        query: &RunQueryParams,
        raw_body: &str,
        body: &Value,
    ) -> Result<SdkResponse, Error> {
        let sig = headers.signature();
        if self.mode == Kind::Cloud {
            if self.signing_key.is_none() {
                return Err(basic_error!(
                    "no signing key available for verifying request signature"
                ));
            }

            let Some(sig) = sig.clone() else {
                return Err(basic_error!("no signature provided for SDK in Cloud mode"));
            };

            self.verify_signature(&sig, raw_body)?;
        } else if let Some(sig) = sig.clone() {
            if self.signing_key.is_some() {
                self.verify_signature(&sig, raw_body)?;
            }
        }

        // find the specified function
        let Some(func) = self.funcs.get(&query.fn_id) else {
            return Err(basic_error!(
                "no function registered as ID: {}",
                &query.fn_id
            ));
        };

        let hydrated_body = self.hydrate_run_body(body).await?;

        func.run(query.clone(), hydrated_body).await
    }
    // run the function

    async fn hydrate_run_body(&self, body: &Value) -> Result<Value, Error> {
        if !body
            .get("use_api")
            .and_then(Value::as_bool)
            .unwrap_or_default()
        {
            return Ok(body.clone());
        }

        let run_id = body
            .get("ctx")
            .and_then(Value::as_object)
            .and_then(|ctx| ctx.get("run_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| basic_error!("missing run_id for use_api payload hydration"))?;

        let events = self.fetch_run_events(run_id).await?;
        let event = events
            .first()
            .cloned()
            .ok_or_else(|| basic_error!("fetched batch events were empty for run {}", run_id))?;
        let steps = self.fetch_run_actions(run_id).await?;

        let mut hydrated = body.clone();
        hydrated["event"] = event;
        hydrated["events"] = Value::Array(events);
        hydrated["steps"] = serde_json::to_value(steps)
            .map_err(|err| basic_error!("error serializing hydrated steps: {}", err))?;
        hydrated["use_api"] = json!(false);

        Ok(hydrated)
    }

    async fn fetch_run_events(&self, run_id: &str) -> Result<Vec<Value>, Error> {
        self.fetch_run_resource(&format!("/v0/runs/{run_id}/batch"))
            .await
    }

    async fn fetch_run_actions(
        &self,
        run_id: &str,
    ) -> Result<HashMap<String, Option<Value>>, Error> {
        self.fetch_run_resource(&format!("/v0/runs/{run_id}/actions"))
            .await
    }

    async fn fetch_run_resource<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let mut response = self
            .send_api_get(path, self.signing_key.as_deref())
            .await
            .map_err(|err| basic_error!("{}", err))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            && self.signing_key.is_some()
            && self.signing_key_fallback.is_some()
        {
            response = self
                .send_api_get(path, self.signing_key_fallback.as_deref())
                .await
                .map_err(|err| basic_error!("{}", err))?;
        }

        let status = response.status();
        if !status.is_success() {
            return Err(basic_error!(
                "error fetching run payload from {}: status {}",
                path,
                status.as_u16()
            ));
        }

        response
            .json::<T>()
            .await
            .map_err(|err| basic_error!("error decoding run payload from {}: {}", path, err))
    }

    async fn send_api_get(
        &self,
        path: &str,
        auth_key: Option<&str>,
    ) -> Result<reqwest::Response, String> {
        let url = format!(
            "{}{}",
            self.inngest.inngest_api_origin().trim_end_matches('/'),
            path
        );
        let mut request = reqwest::Client::new()
            .get(url)
            .header(header::INNGEST_SDK, version::sdk())
            .header(header::INNGEST_REQ_VERSION, EXECUTION_VERSION);

        if let Some(env) = self.inngest.env.clone() {
            request = request.header(header::INNGEST_ENV, env);
        }

        if let Some(key) = auth_key {
            let hashed = Signature::new(key)
                .hash()
                .map_err(|_| "error hashing signing key".to_string())?;
            request = request.header("authorization", format!("Bearer {}", hashed));
        }

        request.send().await.map_err(|err| {
            println!("ERROR: {:?}", err);
            format!("error fetching run payload from {}", path)
        })
    }

    async fn send_sync_request(
        &self,
        sync_url: &str,
        req: &Request,
        query: &SyncQueryParams,
        auth_key: Option<&str>,
    ) -> Result<reqwest::Response, String> {
        let mut sync_req = reqwest::Client::new()
            .post(sync_url)
            .json(req)
            .header(header::INNGEST_SDK, version::sdk())
            .header(header::INNGEST_REQ_VERSION, EXECUTION_VERSION);

        if let Some(env) = self.inngest.env.clone() {
            sync_req = sync_req.header(header::INNGEST_ENV, env);
        }

        if let Some(key) = auth_key {
            let hashed = Signature::new(key)
                .hash()
                .map_err(|_| "error hashing signing key".to_string())?;
            sync_req = sync_req.header("authorization", format!("Bearer {}", hashed));
        }

        if let Some(deploy_id) = query.deploy_id.clone() {
            sync_req = sync_req.query(&[("deployId", &deploy_id)]);
        }

        sync_req.send().await.map_err(|err| {
            println!("ERROR: {:?}", err);
            "error registering".to_string()
        })
    }
}

#[derive(Deserialize, Debug)]
struct RunRequestBody<T: 'static> {
    ctx: RunRequestCtx,
    event: Event<T>,
    events: Vec<Event<T>>,
    #[serde(rename = "use_api")]
    _use_api: bool,
    steps: HashMap<String, Option<Value>>,
    // version: i32,
}

#[derive(Deserialize, Debug)]
struct RunRequestCtx {
    attempt: u8,
    #[serde(default)]
    _disable_immediate_execution: bool,
    env: String,
    // fn_id: String,
    run_id: String,
    // step_id: String,
    #[serde(default)]
    stack: RunRequestCtxStack,
}

#[derive(Default, Deserialize, Debug)]
struct RunRequestCtxStack {
    #[serde(default, rename = "current")]
    _current: u32,
    #[serde(default)]
    stack: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Dev,
    Cloud,
}

const PROBE_SCHEMA_VERSION: &str = "2024-05-24";

#[derive(Serialize)]
#[serde(untagged)]
pub enum IntrospectResult {
    Unauthenticated(Box<IntrospectUnauthedResult>),
    Authenticated(Box<IntrospectAuthedResult>),
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
    InBand(Box<InBandSyncResponse>),
    OutOfBand(Box<OutOfBandSyncResponse>),
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

#[derive(Deserialize, Debug)]
pub struct InngestSyncSuccess {
    #[serde(rename = "ok")]
    _ok: bool,
    modified: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::ServableFn;
    use axum::{
        extract::State,
        http::{HeaderMap, StatusCode, Uri},
        response::IntoResponse,
        routing::{get, post},
        Json, Router,
    };
    use hmac::{Hmac, Mac};
    use serde::{Deserialize, Serialize};
    use sha2::Sha256;
    use std::{
        collections::{HashSet, VecDeque},
        net::TcpListener,
        sync::Arc,
    };
    use tokio::sync::Mutex;

    #[derive(Debug, Deserialize, Serialize)]
    struct FirstEvent {
        message: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct SecondEvent {
        count: u32,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct StepTestError {
        message: String,
    }

    impl std::fmt::Display for StepTestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl std::error::Error for StepTestError {}

    impl From<StepTestError> for Error {
        fn from(err: StepTestError) -> Self {
            Error::Dev(crate::result::DevError::Basic(err.message))
        }
    }

    const PRIMARY_SIGNING_KEY: &str =
        "signkey-test-8ee2262a15e8d3c42d6a840db7af3de2aab08ef632b32a37a687f24b34dba3ff";
    const FALLBACK_SIGNING_KEY: &str =
        "signkey-test-1111111111111111111111111111111111111111111111111111111111111111";

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct SyncRequestRecord {
        authorization: Option<String>,
        env: Option<String>,
        path: String,
        query: Option<String>,
        req_version: Option<String>,
        sdk: Option<String>,
    }

    #[derive(Clone)]
    struct SyncServerState {
        records: Arc<Mutex<Vec<SyncRequestRecord>>>,
        responses: Arc<Mutex<VecDeque<(StatusCode, String)>>>,
    }

    #[tokio::test]
    async fn handler_dispatches_functions_with_different_event_types() {
        let client = Inngest::new("test-app").dev("1");
        let mut handler = Handler::new(&client);

        let first_fn: ServableFn<FirstEvent, Error> = client.create_function(
            FunctionOpts::new("first"),
            Trigger::event("test/first"),
            |input: Input<FirstEvent>, _step| async move {
                Ok(json!({ "message": input.event.data.message }))
            },
        );
        let first_fn_id = first_fn.slug();

        let second_fn: ServableFn<SecondEvent, Error> = client.create_function(
            FunctionOpts::new("second"),
            Trigger::event("test/second"),
            |input: Input<SecondEvent>, _step| async move {
                Ok(json!({ "count": input.event.data.count }))
            },
        );
        let second_fn_id = second_fn.slug();

        handler.register_fns(vec![first_fn.into(), second_fn.into()]);

        let headers = Headers::from(HeaderMap::new());

        let first_body = json!({
            "ctx": { "attempt": 1, "env": "test", "run_id": "run-1" },
            "event": {
                "id": null,
                "name": "test/first",
                "data": { "message": "hello" },
                "ts": null,
                "v": null
            },
            "events": [],
            "use_api": false,
            "steps": {}
        });
        let first_response = handler
            .run(
                &headers,
                &run_query(first_fn_id),
                &first_body.to_string(),
                &first_body,
            )
            .await
            .expect("first function should run");
        assert_eq!(first_response.status, 200);
        assert_eq!(first_response.body, json!({ "message": "hello" }));

        let second_body = json!({
            "ctx": { "attempt": 1, "env": "test", "run_id": "run-2" },
            "event": {
                "id": null,
                "name": "test/second",
                "data": { "count": 42 },
                "ts": null,
                "v": null
            },
            "events": [],
            "use_api": false,
            "steps": {}
        });
        let second_response = handler
            .run(
                &headers,
                &run_query(second_fn_id),
                &second_body.to_string(),
                &second_body,
            )
            .await
            .expect("second function should run");
        assert_eq!(second_response.status, 200);
        assert_eq!(second_response.body, json!({ "count": 42 }));
    }

    #[tokio::test]
    async fn handler_dispatches_functions_registered_via_mixed_apis() {
        let client = Inngest::new("test-app").dev("1");
        let mut handler = Handler::new(&client);

        let first_fn: ServableFn<FirstEvent, Error> = client.create_function(
            FunctionOpts::new("first"),
            Trigger::event("test/first"),
            |input: Input<FirstEvent>, _step| async move {
                Ok(json!({ "message": input.event.data.message }))
            },
        );
        let first_fn_id = first_fn.slug();
        handler.register_fn(first_fn);

        let second_fn: ServableFn<SecondEvent, Error> = client.create_function(
            FunctionOpts::new("second"),
            Trigger::event("test/second"),
            |input: Input<SecondEvent>, _step| async move {
                Ok(json!({ "count": input.event.data.count }))
            },
        );
        let second_fn_id = second_fn.slug();
        handler.register_fns(vec![second_fn.into()]);

        let headers = Headers::from(HeaderMap::new());

        let first_response = handler
            .run(
                &headers,
                &run_query(first_fn_id),
                &event_body("test/first", json!({ "message": "hello" })).to_string(),
                &event_body("test/first", json!({ "message": "hello" })),
            )
            .await
            .expect("first function should run");
        assert_eq!(first_response.body, json!({ "message": "hello" }));

        let second_response = handler
            .run(
                &headers,
                &run_query(second_fn_id),
                &event_body("test/second", json!({ "count": 42 })).to_string(),
                &event_body("test/second", json!({ "count": 42 })),
            )
            .await
            .expect("second function should run");
        assert_eq!(second_response.body, json!({ "count": 42 }));
    }

    #[tokio::test]
    async fn handler_returns_error_for_unknown_function_id() {
        let client = Inngest::new("test-app").dev("1");
        let handler = Handler::new(&client);
        let headers = Headers::from(HeaderMap::new());
        let body = event_body("test/first", json!({ "message": "hello" }));

        let error = match handler
            .run(
                &headers,
                &run_query("test-app-missing".to_string()),
                &body.to_string(),
                &body,
            )
            .await
        {
            Ok(_) => panic!("missing function id should fail"),
            Err(error) => error,
        };

        match error {
            Error::Dev(crate::result::DevError::Basic(message)) => {
                assert!(message.contains("no function registered as ID"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn handler_returns_error_for_mismatched_event_payload() {
        let client = Inngest::new("test-app").dev("1");
        let mut handler = Handler::new(&client);

        let first_fn: ServableFn<FirstEvent, Error> = client.create_function(
            FunctionOpts::new("first"),
            Trigger::event("test/first"),
            |input: Input<FirstEvent>, _step| async move {
                Ok(json!({ "message": input.event.data.message }))
            },
        );
        let first_fn_id = first_fn.slug();
        handler.register_fn(first_fn);

        let headers = Headers::from(HeaderMap::new());
        let body = event_body("test/first", json!({ "count": 42 }));

        let error = match handler
            .run(&headers, &run_query(first_fn_id), &body.to_string(), &body)
            .await
        {
            Ok(_) => panic!("mismatched payload should fail"),
            Err(error) => error,
        };

        match error {
            Error::Dev(crate::result::DevError::Basic(message)) => {
                assert!(message.contains("error parsing run request"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn sync_payload_includes_all_registered_functions() {
        let client = Inngest::new("test-app");
        let mut handler = Handler::new(&client);

        let first_fn: ServableFn<FirstEvent, Error> = client.create_function(
            FunctionOpts::new("first"),
            Trigger::event("test/first"),
            |input: Input<FirstEvent>, _step| async move {
                Ok(json!({ "message": input.event.data.message }))
            },
        );
        let second_fn: ServableFn<SecondEvent, Error> = client.create_function(
            FunctionOpts::new("second"),
            Trigger::event("test/second"),
            |input: Input<SecondEvent>, _step| async move {
                Ok(json!({ "count": input.event.data.count }))
            },
        );

        handler.register_fns(vec![first_fn.into(), second_fn.into()]);

        let payload = handler.sync_payload(&Headers::from(HeaderMap::new()), "axum");
        let function_ids: HashSet<String> = payload
            .functions
            .into_iter()
            .map(|function| function.id)
            .collect();

        assert_eq!(payload.app_name, "test-app");
        assert_eq!(payload.framework, "axum");
        assert_eq!(payload.v, "0.1");
        assert_eq!(payload.sdk, crate::version::sdk());
        assert_eq!(payload.url, "http://127.0.0.1:3000/api/inngest");
        assert_eq!(function_ids.len(), 2);
        assert!(function_ids.contains("test-app-first"));
        assert!(function_ids.contains("test-app-second"));
    }

    #[test]
    fn sync_payload_uses_spec_step_runtime_url_shape() {
        let client = Inngest::new("test-app");
        let mut handler = Handler::new(&client);

        let first_fn: ServableFn<FirstEvent, Error> = client.create_function(
            FunctionOpts::new("first"),
            Trigger::event("test/first"),
            |_input: Input<FirstEvent>, _step| async move { Ok(json!({ "ok": true })) },
        );

        handler.register_fn(first_fn);

        let payload = handler.sync_payload(&Headers::from(HeaderMap::new()), "axum");
        let function = payload
            .functions
            .into_iter()
            .next()
            .expect("registered function should be present");
        let step = function
            .steps
            .get("step")
            .expect("default step should be present");

        assert_eq!(
            step.runtime.url,
            "http://127.0.0.1:3000/api/inngest?fnId=test-app-first&stepId=step"
        );
        assert!(!payload.url.contains("deployId="));
    }

    #[tokio::test]
    async fn cloud_mode_requires_primary_signing_key_even_without_signature() {
        let (handler, fn_id) = registered_handler(Inngest::new("test-app"), None, None);
        let headers = Headers::from(HeaderMap::new());
        let body = event_body("test/first", json!({ "message": "hello" }));

        let error = match handler
            .run(&headers, &run_query(fn_id), &body.to_string(), &body)
            .await
        {
            Ok(_) => panic!("cloud mode should reject missing primary signing key"),
            Err(error) => error,
        };

        assert_basic_error(
            error,
            "no signing key available for verifying request signature",
        );
    }

    #[tokio::test]
    async fn cloud_mode_requires_signature_even_if_request_claims_dev() {
        let (handler, fn_id) =
            registered_handler(Inngest::new("test-app"), Some(PRIMARY_SIGNING_KEY), None);
        let headers = headers(&[(header::INNGEST_SERVER_KIND, "dev")]);
        let body = event_body("test/first", json!({ "message": "hello" }));

        let error = match handler
            .run(&headers, &run_query(fn_id), &body.to_string(), &body)
            .await
        {
            Ok(_) => panic!("cloud mode should still require a signature"),
            Err(error) => error,
        };

        assert_basic_error(error, "no signature provided for SDK in Cloud mode");
    }

    #[tokio::test]
    async fn dev_mode_allows_unsigned_requests_even_if_request_claims_cloud() {
        let client = Inngest::new("test-app").dev("1");
        let (handler, fn_id) = registered_handler(client, None, None);
        let headers = headers(&[(header::INNGEST_SERVER_KIND, "cloud")]);
        let body = event_body("test/first", json!({ "message": "hello" }));

        let response = handler
            .run(&headers, &run_query(fn_id), &body.to_string(), &body)
            .await
            .expect("dev mode should allow unsigned requests");

        assert_eq!(response.status, 200);
    }

    #[tokio::test]
    async fn dev_mode_validates_present_signatures_when_primary_key_exists() {
        let client = Inngest::new("test-app").dev("1");
        let (handler, fn_id) = registered_handler(client, Some(PRIMARY_SIGNING_KEY), None);
        let body = event_body("test/first", json!({ "message": "hello" }));
        let headers = headers(&[(header::INNGEST_SIGNATURE, "t=1&s=deadbeef")]);

        let error = match handler
            .run(&headers, &run_query(fn_id), &body.to_string(), &body)
            .await
        {
            Ok(_) => panic!("invalid signatures should fail in dev mode when a key is configured"),
            Err(error) => error,
        };

        assert_basic_error(error, "sig");
    }

    #[tokio::test]
    async fn run_uses_fallback_signing_key_when_primary_verification_fails() {
        let (handler, fn_id) = registered_handler(
            Inngest::new("test-app"),
            Some(PRIMARY_SIGNING_KEY),
            Some(FALLBACK_SIGNING_KEY),
        );
        let body = event_body("test/first", json!({ "message": "hello" }));
        let signature = sign_body(FALLBACK_SIGNING_KEY, &body.to_string());
        let headers = headers(&[(header::INNGEST_SIGNATURE, &signature)]);

        let response = handler
            .run(&headers, &run_query(fn_id), &body.to_string(), &body)
            .await
            .expect("fallback signing key should validate the request");

        assert_eq!(response.status, 200);
    }

    #[tokio::test]
    async fn introspection_returns_authenticated_payload_when_signature_is_valid_in_dev_mode() {
        let client = Inngest::new("test-app")
            .dev("1")
            .api_origin("https://api.example.com")
            .event_api_origin("https://events.example.com")
            .event_key("evt-test")
            .env("branch");
        let (handler, _fn_id) = registered_handler(
            client,
            Some(PRIMARY_SIGNING_KEY),
            Some(FALLBACK_SIGNING_KEY),
        );
        let raw_body = "{\"ok\":true}";
        let signature = sign_body(PRIMARY_SIGNING_KEY, raw_body);
        let headers = headers(&[(header::INNGEST_SIGNATURE, &signature)]);

        let result = handler
            .introspect(&headers, "axum", raw_body)
            .await
            .expect("introspection should succeed");

        match result {
            IntrospectResult::Authenticated(result) => {
                assert!(result.authentication_succeeded);
                assert_eq!(result.api_origin, "https://api.example.com");
                assert_eq!(result.event_api_origin, "https://events.example.com");
                assert_eq!(result.env, Some("branch".to_string()));
                assert!(result.has_signing_key_fallback);
                assert_eq!(result.mode, Kind::Dev);
                assert_eq!(
                    result.signing_key_hash,
                    Signature::new(PRIMARY_SIGNING_KEY).hash().ok()
                );
                assert_eq!(
                    result.signing_key_fallback_hash,
                    Signature::new(FALLBACK_SIGNING_KEY).hash().ok()
                );
            }
            IntrospectResult::Unauthenticated(_) => {
                panic!("valid signatures should return the authenticated schema")
            }
        }
    }

    #[tokio::test]
    async fn introspection_returns_unauthenticated_payload_when_signature_is_invalid() {
        let client = Inngest::new("test-app").dev("1");
        let (handler, _fn_id) = registered_handler(client, Some(PRIMARY_SIGNING_KEY), None);
        let headers = headers(&[(header::INNGEST_SIGNATURE, "t=1&s=deadbeef")]);

        let result = handler
            .introspect(&headers, "axum", "{\"ok\":true}")
            .await
            .expect("introspection should succeed");

        match result {
            IntrospectResult::Unauthenticated(result) => {
                assert_eq!(result.authentication_succeeded, Some(false));
                assert_eq!(result.mode, Kind::Dev);
            }
            IntrospectResult::Authenticated(_) => {
                panic!("invalid signatures should not return the authenticated schema")
            }
        }
    }

    #[tokio::test]
    async fn sync_uses_configured_api_origin_even_when_request_header_says_dev() {
        let (origin, records) =
            spawn_sync_server(vec![(StatusCode::OK, sync_success_body())]).await;
        let client = Inngest::new("test-app").api_origin(&origin);
        let (handler, _fn_id) = registered_handler(client, None, None);
        let headers = headers(&[(header::INNGEST_SERVER_KIND, "dev")]);

        let result = handler
            .sync(
                &headers,
                &SyncQueryParams {
                    deploy_id: Some("deploy-123".to_string()),
                },
                "axum",
            )
            .await
            .expect("sync should use the configured API origin");

        match result {
            SyncResponse::OutOfBand(result) => assert!(result.modified),
            SyncResponse::InBand(_) => panic!("sync should use the out-of-band response"),
        }

        let records = records.lock().await;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].path, "/fn/register");
        assert_eq!(records[0].query, Some("deployId=deploy-123".to_string()));
    }

    #[tokio::test]
    async fn sync_retries_with_fallback_signing_key_and_sets_protocol_headers() {
        let (origin, records) = spawn_sync_server(vec![
            (
                StatusCode::UNAUTHORIZED,
                "{\"error\":\"unauthorized\"}".to_string(),
            ),
            (StatusCode::OK, sync_success_body()),
        ])
        .await;
        let client = Inngest::new("test-app").api_origin(&origin).env("branch");
        let (handler, _fn_id) = registered_handler(
            client,
            Some(PRIMARY_SIGNING_KEY),
            Some(FALLBACK_SIGNING_KEY),
        );

        let result = handler
            .sync(
                &Headers::from(HeaderMap::new()),
                &SyncQueryParams { deploy_id: None },
                "axum",
            )
            .await
            .expect("sync should retry with the fallback signing key");

        match result {
            SyncResponse::OutOfBand(result) => assert!(result.modified),
            SyncResponse::InBand(_) => panic!("sync should use the out-of-band response"),
        }

        let records = records.lock().await;
        assert_eq!(records.len(), 2);
        assert_eq!(
            records[0].authorization,
            Some(format!(
                "Bearer {}",
                Signature::new(PRIMARY_SIGNING_KEY)
                    .hash()
                    .expect("primary signing key should hash"),
            ))
        );
        assert_eq!(
            records[1].authorization,
            Some(format!(
                "Bearer {}",
                Signature::new(FALLBACK_SIGNING_KEY)
                    .hash()
                    .expect("fallback signing key should hash"),
            ))
        );
        assert_eq!(records[0].sdk, Some(crate::version::sdk()));
        assert_eq!(
            records[0].req_version,
            Some(crate::version::EXECUTION_VERSION.to_string())
        );
        assert_eq!(records[0].env, Some("branch".to_string()));
        assert_eq!(records[1].env, Some("branch".to_string()));
    }

    #[tokio::test]
    async fn handler_passes_requested_step_id_to_function_context() {
        let client = Inngest::new("test-app").dev("1");
        let mut handler = Handler::new(&client);
        let func: ServableFn<FirstEvent, Error> =
            client.create_function(
                FunctionOpts::new("step-id"),
                Trigger::event("test/first"),
                |input: Input<FirstEvent>, _step| async move {
                    Ok(json!({ "step_id": input.ctx.step_id }))
                },
            );
        let fn_id = func.slug();
        handler.register_fn(func);

        let body = event_body("test/first", json!({ "message": "hello" }));
        let response = handler
            .run(
                &Headers::from(HeaderMap::new()),
                &RunQueryParams {
                    fn_id,
                    step_id: "custom-step".to_string(),
                },
                &body.to_string(),
                &body,
            )
            .await
            .expect("handler should pass the requested step id through");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, json!({ "step_id": "custom-step" }));
    }

    #[tokio::test]
    async fn targeted_step_requests_execute_the_matching_hashed_step() {
        let client = Inngest::new("test-app").dev("1");
        let mut handler = Handler::new(&client);
        let func: ServableFn<FirstEvent, Error> = client.create_function(
            FunctionOpts::new("targeted"),
            Trigger::event("test/first"),
            |_input: Input<FirstEvent>, step| async move {
                let value: Value = step
                    .run("hello", || async move {
                        Ok::<_, StepTestError>(json!({ "ok": true }))
                    })
                    .await?;
                Ok(value)
            },
        );
        let fn_id = func.slug();
        handler.register_fn(func);

        let body = event_body("test/first", json!({ "message": "hello" }));
        let targeted_step_id = hash_step_id("hello");
        let response = handler
            .run(
                &Headers::from(HeaderMap::new()),
                &RunQueryParams {
                    fn_id,
                    step_id: targeted_step_id.clone(),
                },
                &body.to_string(),
                &body,
            )
            .await
            .expect("matching targeted step should execute");

        assert_eq!(response.status, 206);
        assert_eq!(response.body[0]["id"], targeted_step_id);
        assert_eq!(response.body[0]["op"], "StepRun");
        assert_eq!(response.body[0]["data"], json!({ "ok": true }));
    }

    #[tokio::test]
    async fn targeted_step_requests_return_step_not_found_for_other_steps() {
        let client = Inngest::new("test-app").dev("1");
        let mut handler = Handler::new(&client);
        let func: ServableFn<FirstEvent, Error> = client.create_function(
            FunctionOpts::new("targeted-miss"),
            Trigger::event("test/first"),
            |_input: Input<FirstEvent>, step| async move {
                let _value: Value = step
                    .run("hello", || async move {
                        Ok::<_, StepTestError>(json!({ "ok": true }))
                    })
                    .await?;
                Ok(json!({ "unexpected": true }))
            },
        );
        let fn_id = func.slug();
        handler.register_fn(func);

        let body = event_body("test/first", json!({ "message": "hello" }));
        let missing_step_id = hash_step_id("other");
        let response = handler
            .run(
                &Headers::from(HeaderMap::new()),
                &RunQueryParams {
                    fn_id,
                    step_id: missing_step_id.clone(),
                },
                &body.to_string(),
                &body,
            )
            .await
            .expect("missing targeted steps should yield step-not-found");

        assert_eq!(response.status, 206);
        assert_eq!(
            response.body,
            json!([{ "id": missing_step_id, "op": "StepNotFound" }])
        );
    }

    #[tokio::test]
    async fn use_api_requests_fetch_events_and_steps_with_auth_headers() {
        let (origin, records) = spawn_run_api_server().await;
        let client = Inngest::new("test-app").dev(&origin).env("branch");
        let mut handler = Handler::new(&client).signing_key(PRIMARY_SIGNING_KEY);

        let func: ServableFn<FirstEvent, Error> = client.create_function(
            FunctionOpts::new("hydrate"),
            Trigger::event("test/first"),
            |input: Input<FirstEvent>, step| async move {
                let memoized: String = step
                    .run("memoized-step", || async move {
                        Ok::<_, StepTestError>("rerun".to_string())
                    })
                    .await?;

                Ok(json!({
                    "event_message": input.event.data.message,
                    "events_len": input.events.len(),
                    "memoized": memoized,
                }))
            },
        );
        let fn_id = func.slug();
        handler.register_fn(func);

        let body = json!({
            "ctx": { "attempt": 1, "env": "test", "run_id": "run-1" },
            "event": {
                "id": null,
                "name": "test/first",
                "data": { "message": "placeholder" },
                "ts": null,
                "v": null
            },
            "events": [],
            "use_api": true,
            "steps": {}
        });
        let response = handler
            .run(
                &Headers::from(HeaderMap::new()),
                &run_query(fn_id),
                &body.to_string(),
                &body,
            )
            .await
            .expect("use_api payloads should hydrate from the API");

        assert_eq!(response.status, 200);
        assert_eq!(
            response.body,
            json!({
                "event_message": "from-api",
                "events_len": 2,
                "memoized": "memoized-from-api",
            })
        );

        let records = records.lock().await;
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].path, "/v0/runs/run-1/batch");
        assert_eq!(records[1].path, "/v0/runs/run-1/actions");
        assert_eq!(
            records[0].authorization,
            Some(format!(
                "Bearer {}",
                Signature::new(PRIMARY_SIGNING_KEY)
                    .hash()
                    .expect("primary signing key should hash"),
            ))
        );
        assert_eq!(records[0].sdk, Some(crate::version::sdk()));
        assert_eq!(
            records[0].req_version,
            Some(crate::version::EXECUTION_VERSION.to_string())
        );
        assert_eq!(records[0].env, Some("branch".to_string()));
        assert_eq!(records[1].env, Some("branch".to_string()));
    }

    fn event_body(name: &str, data: Value) -> Value {
        json!({
            "ctx": { "attempt": 1, "env": "test", "run_id": "run-1" },
            "event": {
                "id": null,
                "name": name,
                "data": data,
                "ts": null,
                "v": null
            },
            "events": [],
            "use_api": false,
            "steps": {}
        })
    }

    fn run_query(fn_id: String) -> RunQueryParams {
        RunQueryParams {
            fn_id,
            step_id: default_step_id(),
        }
    }

    fn registered_handler(
        client: Inngest,
        signing_key: Option<&str>,
        signing_key_fallback: Option<&str>,
    ) -> (Handler, String) {
        let mut handler = Handler::new(&client);
        if let Some(key) = signing_key {
            handler = handler.signing_key(key);
        }
        if let Some(key) = signing_key_fallback {
            handler = handler.signing_key_fallback(key);
        }

        let first_fn: ServableFn<FirstEvent, Error> = client.create_function(
            FunctionOpts::new("first"),
            Trigger::event("test/first"),
            |input: Input<FirstEvent>, _step| async move {
                Ok(json!({ "message": input.event.data.message }))
            },
        );
        let fn_id = first_fn.slug();
        handler.register_fn(first_fn);

        (handler, fn_id)
    }

    fn headers(entries: &[(&str, &str)]) -> Headers {
        let mut header_map = HeaderMap::new();
        for (name, value) in entries {
            header_map.insert(
                axum::http::header::HeaderName::from_bytes(name.as_bytes())
                    .expect("header name should parse"),
                value.parse().expect("header value should parse"),
            );
        }

        Headers::from(header_map)
    }

    fn assert_basic_error(error: Error, expected_fragment: &str) {
        match error {
            Error::Dev(crate::result::DevError::Basic(message)) => {
                assert!(message.contains(expected_fragment), "{message}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn sign_body(signing_key: &str, body: &str) -> String {
        type HmacSha256 = Hmac<Sha256>;

        let timestamp = crate::utils::time::now();
        let normalized_key = signing_key
            .splitn(3, '-')
            .nth(2)
            .expect("signing key should include a signkey-*- prefix");
        let mut mac =
            HmacSha256::new_from_slice(normalized_key.as_bytes()).expect("HMAC key should work");
        mac.update(format!("{body}{timestamp}").as_bytes());

        let sum = mac.finalize();
        let signature = base16::encode_lower(&sum.into_bytes());
        format!("t={timestamp}&s={signature}")
    }

    fn hash_step_id(id: &str) -> String {
        let mut hasher = sha1::Sha1::new();
        hasher.update(id.as_bytes());
        base16::encode_lower(&hasher.finalize())
    }

    async fn spawn_sync_server(
        responses: Vec<(StatusCode, String)>,
    ) -> (String, Arc<Mutex<Vec<SyncRequestRecord>>>) {
        let records = Arc::new(Mutex::new(Vec::new()));
        let state = SyncServerState {
            records: Arc::clone(&records),
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
        };

        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener.local_addr().expect("listener addr should exist");
        let app = Router::new()
            .route("/fn/register", post(record_sync_request))
            .with_state(state);

        tokio::spawn(async move {
            axum::Server::from_tcp(listener)
                .expect("server should bind")
                .serve(app.into_make_service())
                .await
                .expect("server should serve");
        });

        (format!("http://{}", addr), records)
    }

    async fn spawn_run_api_server() -> (String, Arc<Mutex<Vec<SyncRequestRecord>>>) {
        async fn fetch_batch(
            State(records): State<Arc<Mutex<Vec<SyncRequestRecord>>>>,
            headers: HeaderMap,
            uri: Uri,
        ) -> impl IntoResponse {
            records.lock().await.push(SyncRequestRecord {
                authorization: header_value(&headers, "authorization"),
                env: header_value(&headers, header::INNGEST_ENV),
                path: uri.path().to_string(),
                query: uri.query().map(|query| query.to_string()),
                req_version: header_value(&headers, header::INNGEST_REQ_VERSION),
                sdk: header_value(&headers, header::INNGEST_SDK),
            });

            Json(json!([
                {
                    "id": "evt-1",
                    "name": "test/first",
                    "data": { "message": "from-api" },
                    "ts": null,
                    "v": null
                },
                {
                    "id": "evt-2",
                    "name": "test/first",
                    "data": { "message": "from-api-2" },
                    "ts": null,
                    "v": null
                }
            ]))
        }

        async fn fetch_actions(
            State(records): State<Arc<Mutex<Vec<SyncRequestRecord>>>>,
            headers: HeaderMap,
            uri: Uri,
        ) -> impl IntoResponse {
            records.lock().await.push(SyncRequestRecord {
                authorization: header_value(&headers, "authorization"),
                env: header_value(&headers, header::INNGEST_ENV),
                path: uri.path().to_string(),
                query: uri.query().map(|query| query.to_string()),
                req_version: header_value(&headers, header::INNGEST_REQ_VERSION),
                sdk: header_value(&headers, header::INNGEST_SDK),
            });

            Json(json!({
                "3ce0995fad5ea2556082d4021be335153233edf9": {
                    "data": "memoized-from-api"
                }
            }))
        }

        let records = Arc::new(Mutex::new(Vec::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener.local_addr().expect("listener addr should exist");
        let app = Router::new()
            .route("/v0/runs/run-1/batch", get(fetch_batch))
            .route("/v0/runs/run-1/actions", get(fetch_actions))
            .with_state(Arc::clone(&records));

        tokio::spawn(async move {
            axum::Server::from_tcp(listener)
                .expect("server should bind")
                .serve(app.into_make_service())
                .await
                .expect("server should serve");
        });

        (format!("http://{}", addr), records)
    }

    async fn record_sync_request(
        State(state): State<SyncServerState>,
        headers: HeaderMap,
        uri: Uri,
    ) -> (StatusCode, String) {
        state.records.lock().await.push(SyncRequestRecord {
            authorization: header_value(&headers, "authorization"),
            env: header_value(&headers, header::INNGEST_ENV),
            path: uri.path().to_string(),
            query: uri.query().map(|query| query.to_string()),
            req_version: header_value(&headers, header::INNGEST_REQ_VERSION),
            sdk: header_value(&headers, header::INNGEST_SDK),
        });

        state
            .responses
            .lock()
            .await
            .pop_front()
            .unwrap_or((StatusCode::OK, sync_success_body()))
    }

    fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string())
    }

    fn sync_success_body() -> String {
        "{\"ok\":true,\"modified\":true}".to_string()
    }
}

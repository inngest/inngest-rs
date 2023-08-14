use axum::{
    body::Body as AxumBody,
    extract::{Host, MatchedPath, Query, State},
    http::Request as AxumRequest,
};
use serde::Deserialize;

use crate::{
    event::Event,
    function::{Function, Step, StepRetry, StepRuntime},
    router::Handler,
    sdk::Request,
};

use std::{any::Any, collections::HashMap, default::Default, sync::Arc};

pub async fn register(
    Host(host): Host,
    path: MatchedPath,
    State(handler): State<Arc<Handler>>,
) -> Result<(), String> {
    let funcs: Vec<Function> = handler
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
                        url: format!(
                            "http://{}{}?fnId={}&step=step",
                            host,
                            path.as_str(),
                            f.slug()
                        ),
                        method: "http".to_string(),
                    },
                    retries: StepRetry { attempts: 3 },
                },
            );

            Function {
                id: f.slug(),
                name: f.name(),
                triggers: vec![f.trigger()],
                steps,
            }
        })
        .collect();

    let req = Request {
        framework: "axum".to_string(),
        functions: funcs,
        url: format!("http://{}{}", host, path.as_str()),
        ..Default::default()
    };

    let client = reqwest::Client::new();

    match client
        .post("http://127.0.0.1:8288/fn/register")
        .json(&req)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err("error".to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct InvokeQuery {
    #[serde(rename = "fnId")]
    fn_id: String,
    step: String,
}

#[derive(Debug, Deserialize)]
pub struct InvokeCtxStack {
    pub current: u8,
    pub stack: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct InvokeCtx {
    pub attempt: u8,
    pub env: String,
    pub fn_id: String,
    pub run_id: String,
    pub step_id: String,
}

#[derive(Debug, Deserialize)]
pub struct InvokeBody {
    pub ctx: InvokeCtx,
    pub event: Box<Event<_, _>>,
    pub events: Option<Vec<Box<Event<_, _>>>>,
    pub steps: Option<HashMap<String, Box<dyn Any>>>,
    pub use_api: bool,
}

pub async fn invoke(
    request: AxumRequest<AxumBody>,
    State(handler): State<Arc<Handler>>,
    Query(query): Query<InvokeQuery>,
) -> Result<(), String> {
    println!("Request: {:#?}", request);
    println!("Handker: {:#?}", handler);
    println!("Query: {:#?}", query);

    match handler.funcs.iter().find(|f| f.slug() == query.fn_id) {
        None => Err(format!("no function registered as ID: {}", query.fn_id)),
        Some(func) => {
            // println!("Func: {:?}", func.func());

            Ok(())
        }
    }
}

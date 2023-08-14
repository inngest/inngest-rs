use axum::{
    extract::{Host, MatchedPath, Query, State},
    Json,
};
use serde::Deserialize;

use crate::{
    function::{Function, Step, StepRetry, StepRuntime},
    router::Handler,
    sdk::Request,
};

use std::{collections::HashMap, default::Default, sync::Arc};

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

pub async fn invoke(
    Query(query): Query<InvokeQuery>,
    State(handler): State<Arc<Handler>>,
    Json(body): Json<serde_json::Value>,
) -> Result<(), String> {
    println!("Handker: {:#?}", handler);
    println!("Query: {:#?}", query);
    println!("Body: {:#?}", body);

    match handler.funcs.iter().find(|f| f.slug() == query.fn_id) {
        None => Err(format!("no function registered as ID: {}", query.fn_id)),
        Some(func) => {
            // println!("Func: {:?}", func.func());

            Ok(())
        }
    }
}

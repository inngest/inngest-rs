use crate::{
    function::{Function, ServableFunction, Step, StepRetry, StepRuntime},
    router::{Handler, InvokeBody, InvokeQuery},
    sdk::Request,
};
use axum::{
    extract::{Host, MatchedPath, Query, State},
    Json,
};
use serde_json::Value;
use std::{collections::HashMap, default::Default, sync::Arc};

pub async fn register<F: ServableFunction>(
    Host(host): Host,
    path: MatchedPath,
    State(handler): State<Arc<Handler<F>>>,
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

pub async fn invoke<F: ServableFunction>(
    Query(query): Query<InvokeQuery>,
    State(handler): State<Arc<Handler<F>>>,
    Json(body): Json<Value>,
) -> Result<(), String> {
    println!("Body: {:#?}", body);

    match handler.funcs.iter().find(|f| f.slug() == query.fn_id) {
        None => Err(format!("no function registered as ID: {}", query.fn_id)),
        Some(func) => {
            println!("Name: {}", func.name());
            println!("Slug: {}", func.slug());
            println!("Trigger: {:?}", func.trigger());
            println!("Event: {:?}", func.event());

            match serde_json::from_value::<InvokeBody<F::T>>(body) {
                Ok(body) => {
                    println!("{:#?}", body);
                    Ok(())
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    Ok(())
                }
            }
        }
    }
}

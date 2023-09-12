use axum::{
    extract::{Query, State},
    Json,
};
use serde_json::Value;

use crate::{
    function::{Function, Step, StepRetry, StepRuntime},
    router::Handler,
    sdk::Request,
};

use std::{collections::HashMap, default::Default, sync::Arc};

use super::InvokeQuery;

pub async fn register(State(handler): State<Arc<Handler>>) -> Result<(), String> {
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
                        url: "http://127.0.0.1:3000/api/inngest?fnId=dummy-func&step=step"
                            .to_string(),
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
        url: "http://127.0.0.1:3000/api/inngest".to_string(),
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

pub async fn invoke(
    Query(query): Query<InvokeQuery>,
    State(handler): State<Arc<Handler>>,
    Json(body): Json<Value>,
) -> Result<(), String> {
    println!("Body: {:#?}", &body);

    let evt_name = &body["event"]["name"]
        .as_str()
        .expect("event payload should always have 'name'");

    match handler.funcs.iter().find(|f| f.slug() == query.fn_id) {
        None => Err(format!("no function registered as ID: {}", query.fn_id)),
        Some(func) => {
            let mut evt_meta = crate::__private::EventMeta::new();

            // println!("Name: {}", func.name());
            // println!("Slug: {}", func.slug());
            // println!("Trigger: {:?}", func.trigger());

            for meta in crate::__private::inventory::iter::<crate::__private::EventMeta> {
                if &meta.ename == evt_name {
                    evt_meta = meta.clone();
                    break;
                }
            }

            if evt_meta.is_empty() {
                return Err(format!("Event '{}' is not registered", evt_name));
            }

            Ok(())
            // match serde_json::from_value::<InvokeBody<F::T>>(body) {
            //     Ok(body) => {
            //         println!("{:#?}", body);
            //         Ok(())
            //     }
            //     Err(err) => {
            //         println!("Error: {:?}", err);
            //         Ok(())
            //     }
            // }
        }
    }
}

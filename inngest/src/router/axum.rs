use axum::{
    extract::{Query, State},
    Json,
};
use serde_json::Value;

use crate::{
    event::InngestEvent, function::{Function, Step, StepRetry, StepRuntime}, router::Handler, sdk::Request
};

use std::{collections::HashMap, default::Default, sync::Arc};

use super::InvokeQuery;

pub async fn register<T: InngestEvent>(
    State(handler): State<Arc<Handler<T>>>
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
                        url: "http://127.0.0.1:3000/api/inngest?fnId=dummy-func&step=step"
                            .to_string(),
                        method: "http".to_string(),
                    },
                    retries: StepRetry { attempts: 3 },
                },
            );

            Function {
                id: f.slug(),
                name: f.slug(), // TODO: use the proper name
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

    client
        .post("http://127.0.0.1:8288/fn/register")
        .json(&req)
        .send()
        .await
        .map(|_| ())
        .map_err(|_| "error".to_string())
}

pub async fn invoke<T: InngestEvent>(
    Query(query): Query<InvokeQuery>,
    State(handler): State<Arc<Handler<T>>>,
    Json(body): Json<Value>,
) -> Result<(), String> {
    println!("Body: {:#?}", body);

    match handler.funcs.iter().find(|f| f.slug() == query.fn_id) {
        None => Err(format!("no function registered as ID: {}", query.fn_id)),
        Some(func) => {
            println!("Slug: {}", func.slug());
            println!("Trigger: {:?}", func.trigger());
            println!("Event: {:?}", func.event(&body["event"]));

            // println!("Event: {:?}", func.event());

            // match (func.func)() {
            //     Ok(_) => {
            //         println!("OK!!");
            //     }
            //     Err(err) => {
            //         println!("{:?}", err)
            //     }
            // }

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

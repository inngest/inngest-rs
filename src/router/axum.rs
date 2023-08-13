use axum::extract::State;

use crate::{
    function::{Function, Step, StepRetry, StepRuntime},
    router::Handler,
    sdk::Request,
};

use std::{collections::HashMap, default::Default, sync::Arc};

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

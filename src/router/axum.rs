use crate::{
    function::{Function, Step, StepRetry, StepRuntime, Trigger},
    sdk::Request,
};

use std::{collections::HashMap, default::Default};

pub async fn register() -> Result<(), String> {
    let mut steps = HashMap::new();
    steps.insert(
        "step".to_string(),
        Step {
            id: "step".to_string(),
            name: "step".to_string(),
            runtime: StepRuntime {
                url: "http://127.0.0.1:3000/api/inngest?fnId=dummy-func&step=step".to_string(),
                method: "http".to_string(),
            },
            retries: StepRetry { attempts: 3 },
        },
    );

    let func = Function {
        id: "dummy-func".to_string(),
        name: "Dummy func".to_string(),
        triggers: vec![Trigger::EventTrigger {
            event: "test/event".to_string(),
            expression: None,
        }],
        steps,
    };

    let req = Request {
        framework: "axum".to_string(),
        functions: vec![func],
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

use std::collections::HashMap;

use axum::{
    routing::{get, put},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct App {
    #[serde(rename = "appName")]
    app_name: String,
    #[serde(rename = "deployType")]
    deploy_type: String,
    url: String,
    v: String,
    sdk: String,
    framework: String,
    functions: Vec<Function>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Trigger {
    event: Option<String>,
    expression: Option<String>,
    // cron: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Function {
    id: String,
    name: String,
    triggers: Vec<Trigger>,
    steps: HashMap<String, FunctionStep>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct FunctionStep {
    id: String,
    name: String,
    runtime: FunctionStepRuntime,
    retries: FunctionStepRetry,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct FunctionStepRetry {
    attempts: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct FunctionStepRuntime {
    url: String,
    #[serde(rename = "type")]
    method: String,
}

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/api/inngest", put(register).post(invoke));

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn register() -> Result<(), String> {
    let client = reqwest::Client::new();
    let mut steps = HashMap::new();
    steps.insert(
        "step".to_string(),
        FunctionStep {
            id: "step".to_string(),
            name: "step".to_string(),
            runtime: FunctionStepRuntime {
                url: "http://127.0.0.1:3000/api/inngest?fnId=dummy-func&step=step".to_string(),
                method: "http".to_string(),
            },
            retries: FunctionStepRetry { attempts: 3 },
        },
    );

    let func = Function {
        id: "dummy-func".to_string(),
        name: "Dummy func".to_string(),
        triggers: vec![Trigger {
            event: Some("test/event".to_string()),
            expression: None,
            // cron: None,
        }],
        steps,
    };

    let payload = App {
        app_name: "InngestApp".to_string(),
        deploy_type: "ping".to_string(),
        url: "http://127.0.0.1:3000/api/inngest".to_string(),
        v: "1".to_string(),
        sdk: "rust:v0.0.1".to_string(),
        framework: "axum".to_string(),
        functions: vec![func],
    };

    println!("Payload: {:#?}", json!(payload));

    match client
        .post("http://127.0.0.1:8288/fn/register")
        .json(&payload)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err("error".to_string()),
    }
}

async fn invoke() -> &'static str {
    "Invoke"
}

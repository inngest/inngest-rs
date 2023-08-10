use axum::{
    routing::{get, put},
    Router,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
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

#[derive(Clone, Deserialize, Serialize)]
struct Trigger {
    event: Option<String>,
    expression: Option<String>,
    cron: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Function {
    id: String,
    name: String,
    triggers: Vec<Trigger>,
    steps: Vec<FunctionStep>,
}

#[derive(Clone, Deserialize, Serialize)]
struct FunctionStep {
    id: String,
    name: String,
    run_time: FunctionStepRuntime,
    retries: FunctionStepRetry,
}

#[derive(Clone, Deserialize, Serialize)]
struct FunctionStepRetry {
    attempts: u8,
}

#[derive(Clone, Deserialize, Serialize)]
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
    let func = Function {
        id: "dummy-func".to_string(),
        name: "Dummy func".to_string(),
        triggers: vec![],
        steps: vec![],
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

    match client
        .post("http://127.0.0.1:8288")
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

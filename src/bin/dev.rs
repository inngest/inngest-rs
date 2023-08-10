use axum::{
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
struct App {
    #[serde(rename = "appName")]
    app_name: String,
    url: String,
    v: String,
    sdk: String,
    framework: String,
    functions: Vec<Function>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Function {}

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

async fn register() -> Json<App> {
    let payload = App {
        app_name: "InngestApp".to_string(),
        url: "http://127.0.0.1:3000/api/inngest".to_string(),
        v: "1".to_string(),
        sdk: "rust:v0.0.1".to_string(),
        framework: "rust".to_string(),
        functions: vec![],
    };

    Json(payload)
}

async fn invoke() -> &'static str {
    "Invoke"
}

use axum::{
    routing::{get, put},
    Router,
};

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

async fn register() -> &'static str {
    "Register"
}

async fn invoke() -> &'static str {
    "Invoke"
}

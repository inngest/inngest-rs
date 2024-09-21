use axum::{
    routing::{get, put},
    Router,
};
use inngest::{
    function::{create_function, FunctionOps, Input, ServableFn, Trigger},
    handler::Handler,
    serve, Inngest,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let client = Inngest::new("rust-dev");
    let mut inngest_handler = Handler::new(client);
    inngest_handler.register_fn(dummy_fn());
    inngest_handler.register_fn(hello_fn());

    let inngest_state = Arc::new(inngest_handler);

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route(
            "/api/inngest",
            put(serve::axum::register).post(serve::axum::invoke),
        )
        .with_state(inngest_state);

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Serialize, Deserialize, Debug)]
struct TestData {
    name: String,
    data: u8,
}

fn dummy_fn() -> ServableFn<TestData> {
    create_function(
        FunctionOps {
            id: "Dummy func".to_string(),
            ..Default::default()
        },
        Trigger::EventTrigger {
            event: "test/event".to_string(),
            expression: None,
        },
        |input: &Input<TestData>| {
            println!("In dummy function");

            let evt = &input.event;
            println!("Event: {}", evt.name);

            Ok(json!({ "dummy": true }))
        },
    )
}

fn hello_fn() -> ServableFn<TestData> {
    create_function(
        FunctionOps {
            id: "Hello func".to_string(),
            ..Default::default()
        },
        Trigger::EventTrigger {
            event: "test/hello".to_string(),
            expression: None,
        },
        |input: &Input<TestData>| {
            println!("In hello function");

            let evt = &input.event;
            println!("Event: {}", evt.name);

            Ok(json!("test hello"))
        },
    )
}

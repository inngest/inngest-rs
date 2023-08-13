use std::sync::Arc;

use axum::{
    routing::{get, put},
    Router,
};

use inngest::{
    function::{create_function, FunctionOps, Input, ServableFunction, Trigger},
    router::{axum as inngest_axum, Handler},
};

#[tokio::main]
async fn main() {
    let mut inngest_handler = Handler::new();
    inngest_handler.register_fn(dummy_fn());

    let inngest_state = Arc::new(inngest_handler);

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route(
            "/api/inngest",
            put(inngest_axum::register).post(inngest_axum::invoke),
        )
        .with_state(inngest_state);

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

struct DummyEvent {}

fn dummy_fn() -> impl ServableFunction {
    create_function(
        FunctionOps {
            name: "Dummy func".to_string(),
            ..Default::default()
        },
        Trigger::EventTrigger {
            event: "test/event".to_string(),
            expression: None,
        },
        |_input: Input<DummyEvent>| {
            println!("In dummy function");

            Ok(Box::new(()))
        },
    )
}

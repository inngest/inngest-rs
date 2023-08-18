mod func;

use axum::{
    routing::{get, put},
    Router,
};
use inngest::router::{axum as inngest_axum, Handler};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let mut inngest_handler = Handler::new();
    inngest_handler.register_fn(func::dummy_fn());
    inngest_handler.register_fn(func::hello_fn());

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

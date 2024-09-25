use axum::{
    routing::{get, put},
    Router,
};
use inngest::{
    function::{create_function, FunctionOps, Input, ServableFn, Trigger},
    handler::Handler,
    result::InggestError,
    serve,
    step_tool::Step as StepTool,
    Inngest,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{sync::Arc, time::Duration};

#[tokio::main]
async fn main() {
    let client = Inngest::new("rust-dev");
    let mut inngest_handler = Handler::new(client);
    inngest_handler.register_fn(dummy_fn());
    inngest_handler.register_fn(hello_fn());
    inngest_handler.register_fn(step_run());

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

#[derive(Debug, Deserialize, Serialize)]
enum UserLandError {
    General(String),
}

impl std::fmt::Display for UserLandError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            UserLandError::General(msg) => write!(f, "General error: {}", msg),
        }
    }
}

impl std::error::Error for UserLandError {}

impl From<UserLandError> for inngest::result::InggestError {
    fn from(err: UserLandError) -> inngest::result::InggestError {
        match err {
            UserLandError::General(msg) => inngest::result::InggestError::Basic(msg),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct TestData {
    name: String,
    data: u8,
}

fn dummy_fn() -> ServableFn<TestData, InggestError> {
    create_function(
        FunctionOps {
            id: "Dummy func".to_string(),
            ..Default::default()
        },
        Trigger::EventTrigger {
            event: "test/event".to_string(),
            expression: None,
        },
        |input: &Input<TestData>, step: &mut StepTool| {
            println!("In dummy function");

            let evt = &input.event;
            println!("Event: {}", evt.name);
            step.sleep("sleep-test", Duration::from_secs(10))?;

            Ok(json!({ "dummy": true }))
        },
    )
}

fn hello_fn() -> ServableFn<TestData, InggestError> {
    create_function(
        FunctionOps {
            id: "Hello func".to_string(),
            ..Default::default()
        },
        Trigger::EventTrigger {
            event: "test/hello".to_string(),
            expression: None,
        },
        |input: &Input<TestData>, _step: &mut StepTool| {
            println!("In hello function");

            let evt = &input.event;
            println!("Event: {}", evt.name);

            Ok(json!("test hello"))
        },
    )
}

fn step_run() -> ServableFn<TestData, InggestError> {
    create_function(
        FunctionOps {
            id: "Step run".to_string(),
            ..Default::default()
        },
        Trigger::EventTrigger {
            event: "test/step-run".to_string(),
            expression: None,
        },
        |_input: &Input<TestData>,
         step: &mut StepTool|
         -> Result<serde_json::Value, InggestError> {
            println!("In step run function");
            step.run("some-step-function", || {
                println!("In step function");
                Ok::<_, UserLandError>(json!({ "returned from within step.run": true }))
            })
        },
    )
}

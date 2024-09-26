use axum::{
    routing::{get, put},
    Router,
};
use inngest::{
    event::Event,
    function::{create_function, FunctionOps, Input, ServableFn, Trigger},
    handler::Handler,
    result::InngestError,
    serve,
    step_tool::{InvokeFunctionOpts, Step as StepTool, WaitForEventOpts},
    Inngest,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

#[tokio::main]
async fn main() {
    let client = Inngest::new("rust-dev");
    let mut inngest_handler = Handler::new(client);
    inngest_handler.register_fn(dummy_fn());
    inngest_handler.register_fn(hello_fn());
    inngest_handler.register_fn(step_run());
    inngest_handler.register_fn(fallible_step_run());

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

impl From<UserLandError> for inngest::result::InngestError {
    fn from(err: UserLandError) -> inngest::result::InngestError {
        match err {
            UserLandError::General(msg) => inngest::result::InngestError::Basic(msg),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct TestData {
    name: String,
    data: u8,
}

fn dummy_fn() -> ServableFn<TestData, InngestError> {
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
            step.sleep("sleep-test", Duration::from_secs(3))?;

            let resp: Value = step.invoke("test-invoke", InvokeFunctionOpts{
                function_id: fallible_step_run().slug(),
                data: json!({ "name": "yolo", "data": 200 }),
                timeout: None
            })?;

            println!("Invoke: {:?}", resp);

            let evt: Option<Event<Value>> = step.wait_for_event(
                "wait",
                WaitForEventOpts {
                    event: "test/wait".to_string(),
                    timeout: Duration::from_secs(60),
                    if_exp: None,
                },
            )?;

            println!("Event: {:?}", evt);

            Ok(json!({ "dummy": true }))
        },
    )
}

fn hello_fn() -> ServableFn<TestData, InngestError> {
    create_function(
        FunctionOps {
            id: "Hello func".to_string(),
            ..Default::default()
        },
        Trigger::EventTrigger {
            event: "test/hello".to_string(),
            expression: None,
        },
        |input: &Input<TestData>, step: &mut StepTool| {
            println!("In hello function");

            let evt = &input.event;
            println!("Event: {}", evt.name);

            step.sleep_until("sleep", 1727245659000)?;

            Ok(json!("test hello"))
        },
    )
}

fn step_run() -> ServableFn<TestData, InngestError> {
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
         -> Result<serde_json::Value, InngestError> {
            let some_captured_variable = "captured".to_string();

            let step_res = step.run(
                "some-step-function",
                || -> Result<serde_json::Value, UserLandError> {
                    let captured = some_captured_variable.clone();
                    Ok(json!({ "returned from within step.run": true, "captured": captured }))
                },
            )?;

            // do something with the res..

            Ok(step_res)
        },
    )
}

fn fallible_step_run() -> ServableFn<TestData, InngestError> {
    create_function(
        FunctionOps {
            id: "Fallible Step run".to_string(),
            ..Default::default()
        },
        Trigger::EventTrigger {
            event: "test/step-run-fallible".to_string(),
            expression: None,
        },
        |input: &Input<TestData>, step: &mut StepTool| -> Result<serde_json::Value, InngestError> {
            let step_res = step.run(
                "fallible-step-function",
                || -> Result<serde_json::Value, UserLandError> {
                    // if even, fail
                    if input.ctx.attempt % 2 == 0 {
                        return Err(UserLandError::General(format!(
                            "Attempt {}",
                            input.ctx.attempt
                        )));
                    }

                    Ok(json!({ "returned from within step.run": true }))
                },
            )?;

            // do something with the res..

            Ok(step_res)
        },
    )
}

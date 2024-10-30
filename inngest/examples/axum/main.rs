use axum::{
    routing::get,
    Router,
};
use inngest::{
    client::Inngest,
    event::Event,
    function::{FunctionOpts, Input, ServableFn, Trigger},
    handler::Handler,
    into_dev_result,
    result::{DevError, Error},
    serve,
    step_tool::{InvokeFunctionOpts, Step as StepTool, WaitForEventOpts},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{env, sync::Arc, time::Duration};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let client = Inngest::new("rust-dev");
    let mut inngest_handler = Handler::new(&client);
    inngest_handler.register_fns(vec![
        dummy_fn(&client),
        hello_fn(&client),
        step_run(&client),
        fallible_step_run(&client),
        incorrectly_propagates_error(&client),
    ]);

    let inngest_state = Arc::new(inngest_handler);

    let app = Router::new()
        .route("/", get(|| async { "OK!\n" }))
        .route(
            "/api/inngest",
            get(serve::axum::introspect).put(serve::axum::register).post(serve::axum::invoke),
        )
        .with_state(inngest_state);

    let port = match env::var("PORT") {
        Ok(p) => p,
        Err(_) => "3000".to_string(),
    };

    let addr = format!("[::]:{}", port)
        .parse::<std::net::SocketAddr>()
        .unwrap();

    // run it with hyper on localhost:3000
    axum::Server::bind(&addr)
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

impl From<UserLandError> for inngest::result::DevError {
    fn from(err: UserLandError) -> inngest::result::DevError {
        match err {
            UserLandError::General(msg) => inngest::result::DevError::Basic(msg),
        }
    }
}

impl From<UserLandError> for inngest::result::Error {
    fn from(err: UserLandError) -> inngest::result::Error {
        inngest::result::Error::Dev(err.into())
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum Data {
    TestData { name: String, data: u8 },
    Hello { msg: String },
}

fn dummy_fn(client: &Inngest) -> ServableFn<Data, Error> {
    let invoke_id = fallible_step_run(client).slug();

    client.create_function(
        FunctionOpts::new("dummy-func").name("Dummy func"),
        Trigger::event("test/event"),
        move |_input: Input<Data>, step: StepTool| {
            let function_id = invoke_id.clone();

            async move {
                // let evt = &input.event;
                // println!("Event: {:#?}", evt);
                step.sleep("sleep-test", Duration::from_secs(3))?;

                let _: Value = step.invoke(
                    "test-invoke",
                    InvokeFunctionOpts {
                        function_id,
                        data: json!({ "name": "yolo", "data": 200 }),
                        timeout: None,
                    },
                )?;

                // println!("Invoke: {:?}", resp);

                let evt: Option<Event<Value>> = step.wait_for_event(
                    "wait",
                    WaitForEventOpts {
                        event: "test/wait".to_string(),
                        timeout: Duration::from_secs(60),
                        if_exp: None,
                    },
                )?;

                Ok(json!({ "dummy": true, "evt": evt }))
            }
        },
    )
}

fn hello_fn(client: &Inngest) -> ServableFn<Data, Error> {
    client.create_function(
        FunctionOpts::new("hello-func").name("Hello func"),
        Trigger::event("test/hello"),
        |input: Input<Data>, step: StepTool| async move {
            let step_res = into_dev_result!(
                step.run("fallible-step-function", || async move {
                    // if even, fail
                    if input.ctx.attempt % 2 == 0 {
                        return Err(UserLandError::General(format!(
                            "Attempt {}",
                            input.ctx.attempt
                        )));
                    }

                    Ok(json!({ "returned from within step.run": true }))
                })
                .await
            )?;

            step.sleep("sleep-test", Duration::from_secs(3))?;

            let evt: Option<Event<Value>> = step.wait_for_event(
                "wait",
                WaitForEventOpts {
                    event: "test/wait".to_string(),
                    timeout: Duration::from_secs(60),
                    if_exp: None,
                },
            )?;

            Ok(json!({ "hello": true, "evt": evt, "step": step_res }))
        },
    )
}

async fn call_some_step_function(_input: Input<Data>, step: StepTool) -> Result<Value, Error> {
    let some_captured_variable = "captured".to_string();

    let step_res = step
        .run("some-step-function", || async {
            let captured = some_captured_variable.clone();
            Ok::<_, UserLandError>(
                json!({ "returned from within step.run": true, "captured": captured }),
            )
        })
        .await?;

    // do something with the res..

    Ok(step_res)
}

fn step_run(client: &Inngest) -> ServableFn<Data, Error> {
    client.create_function(
        FunctionOpts::new("step-run").name("Step run"),
        Trigger::event("test/step-run"),
        call_some_step_function,
    )
}

fn incorrectly_propagates_error(client: &Inngest) -> ServableFn<Data, Error> {
    client.create_function(
        FunctionOpts::new("step-run").name("Step run"),
        Trigger::event("test/step-run-incorrect"),
        |_input: Input<Data>, step: StepTool| async move {
            let some_captured_variable = "captured".to_string();

            let res = step
                .run("some-step-function", || async move {
                    let captured = some_captured_variable.clone();
                    Ok::<_, UserLandError>(
                        json!({ "returned from within step.run": true, "captured": captured }),
                    )
                })
                .await;

            match res {
                Ok(res) => Ok(res),
                Err(_) => Ok(Value::String(
                    "returning value instead of error - whooops".to_string(),
                )),
            }
        },
    )
}

fn fallible_step_run(client: &Inngest) -> ServableFn<Data, Error> {
    client.create_function(
        FunctionOpts::new("fallible-step-run").name("Fallible Step run"),
        Trigger::event("test/step-run-fallible"),
        |input: Input<Data>, step: StepTool| async move {
            let step_res = into_dev_result!(
                step.run("fallible-step-function", || async move {
                    // if even, fail
                    if input.ctx.attempt % 2 == 0 {
                        return Err(UserLandError::General(format!(
                            "Attempt {}",
                            input.ctx.attempt
                        )));
                    }

                    Ok(json!({ "returned from within step.run": true }))
                },)
                    .await
            );

            match &step_res {
                Err(err) => match err {
                    DevError::NoRetry(_) => println!("No retry"),
                    DevError::RetryAt(_) => println!("Retry after"),
                    DevError::Basic(msg) => println!("Basic {}", msg),
                },
                Ok(_) => println!("Success"),
            }

            // do something with the res..

            Ok(step_res?)
        },
    )
}

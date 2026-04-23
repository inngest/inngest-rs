mod e2e_support;

use e2e_support::{spawn_app, wait_for_run_status, wait_for_state, DevServer, DevServerLock};
use inngest::{
    client::Inngest,
    event::Event,
    function::{FunctionFailureEvent, FunctionOpts, Input, ServableFn, Trigger},
    result::{DevError, Error},
    step_tool::Step as StepTool,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FailureEventData {
    message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FailureCapture {
    function_id: String,
    original_message: String,
    run_id: String,
    error_message: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_failure_runs_after_a_function_exhausts_retries() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("function-config-e2e-app");
    let event_name = e2e_support::unique_name("test.function.failure");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let failed_run_id = Arc::new(Mutex::new(None::<String>));
    let failure_capture = Arc::new(Mutex::new(None::<FailureCapture>));

    let failed_run_capture = Arc::clone(&failed_run_id);
    let failure_capture_state = Arc::clone(&failure_capture);
    let func: ServableFn<FailureEventData, Error> = client
        .create_function(
            FunctionOpts::new("failure-parent")
                .name("Failure Parent")
                .retries(0),
            Trigger::event(&event_name),
            move |input: Input<FailureEventData>, _step: StepTool| {
                let failed_run_capture = Arc::clone(&failed_run_capture);

                async move {
                    *failed_run_capture.lock().unwrap() = Some(input.ctx.run_id.clone());
                    Err(Error::Dev(DevError::Basic("boom".to_string())))
                }
            },
        )
        .on_failure(
            move |input: Input<FunctionFailureEvent<FailureEventData>>, _step: StepTool| {
                let failure_capture_state = Arc::clone(&failure_capture_state);

                async move {
                    *failure_capture_state.lock().unwrap() = Some(FailureCapture {
                        function_id: input.event.data.function_id.clone(),
                        original_message: input.event.data.event.data.message.clone(),
                        run_id: input.event.data.run_id.clone(),
                        error_message: input.event.data.error.message.clone(),
                    });

                    Ok(json!({ "handled": true }))
                }
            },
        );

    let parent_fn_id = func.slug();
    let app = spawn_app(client.clone(), vec![func.into()]).await;
    app.sync().await;

    client
        .send_event(&Event::new(
            &event_name,
            FailureEventData {
                message: "hello".to_string(),
            },
        ))
        .await
        .expect("failure event should send successfully");

    let run_id = wait_for_state(&failed_run_id, Duration::from_secs(5)).await;
    let run = wait_for_run_status(&run_id, "Failed", Duration::from_secs(20)).await;
    let capture = wait_for_state(&failure_capture, Duration::from_secs(20)).await;

    assert_eq!(run.status, "Failed");
    assert_eq!(
        capture,
        FailureCapture {
            function_id: parent_fn_id,
            original_message: "hello".to_string(),
            run_id,
            error_message: "boom".to_string(),
        }
    );
}

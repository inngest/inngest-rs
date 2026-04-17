mod e2e_support;

use e2e_support::{spawn_app, wait_for_run_status, wait_for_state, DevServer, DevServerLock};
use inngest::{
    client::Inngest,
    event::Event,
    function::{FunctionOpts, Input, ServableFn, Trigger},
    result::Error,
    step_tool::{InvokeFunctionOpts, Step as StepTool},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Debug, Deserialize, Serialize)]
struct EmptyEventData {}

#[derive(Debug, Deserialize, Serialize)]
struct InvokeEventData {
    message: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn step_invoke_returns_child_output() {
    // The dev server uses fixed ports, so each e2e test holds the suite-wide lock.
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("invoke-e2e-app");
    let parent_event_name = e2e_support::unique_name("test.invoke.parent");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let parent_run_id = Arc::new(Mutex::new(None::<String>));
    let invoke_result = Arc::new(Mutex::new(None::<String>));

    let child_fn: ServableFn<InvokeEventData, Error> = client.create_function(
        FunctionOpts::new("invoke-child-fn").name("Invoke Child Fn"),
        Trigger::event("never"),
        |input: Input<InvokeEventData>, _step: StepTool| async move {
            Ok(json!(input.event.data.message))
        },
    );
    let child_fn_id = child_fn.slug();

    let parent_run_state = Arc::clone(&parent_run_id);
    let invoke_result_state = Arc::clone(&invoke_result);
    let parent_fn: ServableFn<EmptyEventData, Error> = client.create_function(
        FunctionOpts::new("invoke-parent-fn").name("Invoke Parent Fn"),
        Trigger::event(&parent_event_name),
        move |input: Input<EmptyEventData>, step: StepTool| {
            let parent_run_state = Arc::clone(&parent_run_state);
            let invoke_result_state = Arc::clone(&invoke_result_state);
            let child_fn_id = child_fn_id.clone();

            async move {
                // Capture the parent run before the invoke step interrupts and resumes the function.
                *parent_run_state.lock().unwrap() = Some(input.ctx.run_id.clone());

                // The dev server returns the child output through the invoke envelope.
                let result: String = step.invoke(
                    "invoke-child",
                    InvokeFunctionOpts {
                        function_id: child_fn_id,
                        data: json!({ "message": "hello-from-child" }),
                        timeout: None,
                    },
                )?;

                *invoke_result_state.lock().unwrap() = Some(result.clone());

                Ok(json!(result))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![child_fn.into(), parent_fn.into()]).await;
    app.sync().await;

    // Trigger the parent through the dev server so the invoke path uses the real control plane.
    client
        .send_event(&Event::new(&parent_event_name, EmptyEventData {}))
        .await
        .expect("parent event should send successfully");

    let run_id = wait_for_state(&parent_run_id, Duration::from_secs(5)).await;
    let run = wait_for_run_status(&run_id, "Completed", Duration::from_secs(15)).await;

    // Both the in-process state capture and the final run output should see the child payload.
    let result = wait_for_state(&invoke_result, Duration::from_secs(5)).await;
    assert_eq!(result, "hello-from-child");
    assert_eq!(run.output, json!("hello-from-child"));
}

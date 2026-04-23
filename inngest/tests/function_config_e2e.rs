mod e2e_support;

use e2e_support::{spawn_app, wait_for_run_status, wait_for_state, DevServer, DevServerLock};
use inngest::{
    client::Inngest,
    event::Event,
    function::{
        FunctionBatchEvents, FunctionFailureEvent, FunctionOpts, FunctionRateLimit, Input,
        ServableFn, Trigger,
    },
    result::{DevError, Error},
    step_tool::Step as StepTool,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct BatchCapture {
    messages: Vec<String>,
    primary_message: String,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_events_delivers_multiple_events_to_one_function_run() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("function-config-batch-app");
    let event_name = e2e_support::unique_name("test.function.batch");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let batch_run_id = Arc::new(Mutex::new(None::<String>));
    let batch_capture = Arc::new(Mutex::new(None::<BatchCapture>));

    let batch_run_state = Arc::clone(&batch_run_id);
    let batch_capture_state = Arc::clone(&batch_capture);
    let func: ServableFn<FailureEventData, Error> = client.create_function(
        FunctionOpts::new("batch-parent")
            .name("Batch Parent")
            .batch_events(FunctionBatchEvents::new(2, Duration::from_secs(2))),
        Trigger::event(&event_name),
        move |input: Input<FailureEventData>, _step: StepTool| {
            let batch_run_state = Arc::clone(&batch_run_state);
            let batch_capture_state = Arc::clone(&batch_capture_state);

            async move {
                *batch_run_state.lock().unwrap() = Some(input.ctx.run_id.clone());

                let messages = input
                    .events
                    .iter()
                    .map(|event| event.data.message.clone())
                    .collect::<Vec<_>>();
                *batch_capture_state.lock().unwrap() = Some(BatchCapture {
                    primary_message: input.event.data.message.clone(),
                    messages: messages.clone(),
                });

                Ok(json!({
                    "primary": input.event.data.message,
                    "messages": messages,
                }))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![func.into()]).await;
    app.sync().await;

    let first = Event::new(
        &event_name,
        FailureEventData {
            message: "first".to_string(),
        },
    );
    let second = Event::new(
        &event_name,
        FailureEventData {
            message: "second".to_string(),
        },
    );

    client
        .send_events(&[&first, &second])
        .await
        .expect("batched events should send successfully");

    let run_id = wait_for_state(&batch_run_id, Duration::from_secs(5)).await;
    let run = wait_for_run_status(&run_id, "Completed", Duration::from_secs(15)).await;
    let capture = wait_for_state(&batch_capture, Duration::from_secs(5)).await;

    assert_eq!(capture.messages.len(), 2);
    assert!(capture.messages.contains(&"first".to_string()));
    assert!(capture.messages.contains(&"second".to_string()));
    assert!(capture.messages.contains(&capture.primary_message));
    assert_eq!(
        run.output,
        json!({
            "primary": capture.primary_message,
            "messages": capture.messages,
        })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rate_limit_allows_only_one_run_within_the_configured_period() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("function-config-rate-limit-app");
    let event_name = e2e_support::unique_name("test.function.rate-limit");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let rate_limited_run_id = Arc::new(Mutex::new(None::<String>));
    let received_messages = Arc::new(Mutex::new(Vec::<String>::new()));
    let invocation_count = Arc::new(AtomicUsize::new(0));

    let rate_limited_run_state = Arc::clone(&rate_limited_run_id);
    let received_messages_state = Arc::clone(&received_messages);
    let invocation_count_state = Arc::clone(&invocation_count);
    let func: ServableFn<FailureEventData, Error> = client.create_function(
        FunctionOpts::new("rate-limit-parent")
            .name("Rate Limit Parent")
            .rate_limit(FunctionRateLimit::new(1, Duration::from_secs(30))),
        Trigger::event(&event_name),
        move |input: Input<FailureEventData>, _step: StepTool| {
            let rate_limited_run_state = Arc::clone(&rate_limited_run_state);
            let received_messages_state = Arc::clone(&received_messages_state);
            let invocation_count_state = Arc::clone(&invocation_count_state);

            async move {
                invocation_count_state.fetch_add(1, Ordering::SeqCst);
                *rate_limited_run_state.lock().unwrap() = Some(input.ctx.run_id.clone());
                received_messages_state
                    .lock()
                    .unwrap()
                    .push(input.event.data.message.clone());

                Ok(json!({ "message": input.event.data.message }))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![func.into()]).await;
    app.sync().await;

    client
        .send_event(&Event::new(
            &event_name,
            FailureEventData {
                message: "first".to_string(),
            },
        ))
        .await
        .expect("first rate-limited event should send successfully");

    let run_id = wait_for_state(&rate_limited_run_id, Duration::from_secs(5)).await;

    client
        .send_event(&Event::new(
            &event_name,
            FailureEventData {
                message: "second".to_string(),
            },
        ))
        .await
        .expect("second rate-limited event should send successfully");

    let run = wait_for_run_status(&run_id, "Completed", Duration::from_secs(15)).await;

    tokio::time::sleep(Duration::from_secs(3)).await;

    assert_eq!(invocation_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        received_messages.lock().unwrap().clone(),
        vec!["first".to_string()]
    );
    assert_eq!(run.output, json!({ "message": "first" }));
}

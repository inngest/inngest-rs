mod e2e_support;

use e2e_support::{spawn_app, wait_for_run_status, wait_for_state, DevServer, DevServerLock};
use inngest::{
    client::Inngest,
    event::Event,
    function::{
        FunctionBatchEvents, FunctionCancel, FunctionConcurrency, FunctionDebounce,
        FunctionFailureEvent, FunctionOpts, FunctionRateLimit, FunctionSingleton,
        FunctionSingletonMode, FunctionThrottle, Input, ServableFn, Trigger,
    },
    result::{DevError, Error},
    step_tool::Step as StepTool,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
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

#[derive(Debug, Deserialize, Serialize)]
struct E2eTestError {
    message: String,
}

impl Display for E2eTestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for E2eTestError {}

impl From<E2eTestError> for Error {
    fn from(err: E2eTestError) -> Self {
        Error::Dev(DevError::Basic(err.message))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct KeyedEventData {
    key: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    data: T,
}

async fn wait_for_value<T: Clone>(
    state: &Arc<Mutex<T>>,
    timeout: Duration,
    predicate: impl Fn(&T) -> bool,
) -> T {
    let deadline = Instant::now() + timeout;

    loop {
        let value = state.lock().unwrap().clone();
        if predicate(&value) {
            return value;
        }

        assert!(Instant::now() < deadline, "timed out waiting for state predicate");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn fetch_run_record(run_id: &str) -> Option<e2e_support::RunRecord> {
    let response = reqwest::Client::new()
        .get(format!("{}/v1/runs/{run_id}", e2e_support::DEV_SERVER_ORIGIN))
        .send()
        .await
        .expect("run lookup should complete");

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return None;
    }

    assert!(
        response.status().is_success(),
        "run lookup failed with status {}",
        response.status()
    );

    let envelope = response
        .json::<ApiEnvelope<e2e_support::RunRecord>>()
        .await
        .expect("run lookup should decode");

    Some(envelope.data)
}

async fn wait_for_run_status_matching(
    run_id: &str,
    timeout: Duration,
    predicate: impl Fn(&str) -> bool,
) -> e2e_support::RunRecord {
    let deadline = Instant::now() + timeout;
    let mut last_status = None::<String>;

    loop {
        if let Some(run) = fetch_run_record(run_id).await {
            if predicate(&run.status) {
                return run;
            }

            last_status = Some(run.status);
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for run {run_id} to reach a matching status; last status was {:?}",
            last_status
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn debounce_runs_once_with_the_latest_event_payload() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("function-config-debounce-app");
    let event_name = e2e_support::unique_name("test.function.debounce");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let run_id_state = Arc::new(Mutex::new(None::<String>));
    let received_messages = Arc::new(Mutex::new(Vec::<String>::new()));
    let invocation_count = Arc::new(AtomicUsize::new(0));

    let run_id_capture = Arc::clone(&run_id_state);
    let received_messages_capture = Arc::clone(&received_messages);
    let invocation_count_capture = Arc::clone(&invocation_count);
    let func: ServableFn<KeyedEventData, Error> = client.create_function(
        FunctionOpts::new("debounce-parent")
            .name("Debounce Parent")
            .debounce(
                FunctionDebounce::new(Duration::from_secs(2))
                    .key("event.data.key")
                    .timeout(Duration::from_secs(4)),
            ),
        Trigger::event(&event_name),
        move |input: Input<KeyedEventData>, _step: StepTool| {
            let run_id_capture = Arc::clone(&run_id_capture);
            let received_messages_capture = Arc::clone(&received_messages_capture);
            let invocation_count_capture = Arc::clone(&invocation_count_capture);

            async move {
                invocation_count_capture.fetch_add(1, Ordering::SeqCst);
                *run_id_capture.lock().unwrap() = Some(input.ctx.run_id.clone());
                received_messages_capture
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
            KeyedEventData {
                key: "group-1".to_string(),
                message: "first".to_string(),
            },
        ))
        .await
        .expect("first debounced event should send successfully");

    tokio::time::sleep(Duration::from_millis(500)).await;

    client
        .send_event(&Event::new(
            &event_name,
            KeyedEventData {
                key: "group-1".to_string(),
                message: "second".to_string(),
            },
        ))
        .await
        .expect("second debounced event should send successfully");

    let run_id = wait_for_state(&run_id_state, Duration::from_secs(10)).await;
    let run = wait_for_run_status(&run_id, "Completed", Duration::from_secs(15)).await;

    assert_eq!(invocation_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        received_messages.lock().unwrap().clone(),
        vec!["second".to_string()]
    );
    assert_eq!(run.output, json!({ "message": "second" }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn singleton_skip_suppresses_a_second_run_for_the_same_key() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("function-config-singleton-app");
    let event_name = e2e_support::unique_name("test.function.singleton");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let started_runs = Arc::new(Mutex::new(HashMap::<String, String>::new()));

    let started_runs_capture = Arc::clone(&started_runs);
    let func: ServableFn<KeyedEventData, Error> = client.create_function(
        FunctionOpts::new("singleton-parent")
            .name("Singleton Parent")
            .singleton(FunctionSingleton::new(FunctionSingletonMode::Skip).key("event.data.key")),
        Trigger::event(&event_name),
        move |input: Input<KeyedEventData>, step: StepTool| {
            let started_runs_capture = Arc::clone(&started_runs_capture);

            async move {
                started_runs_capture
                    .lock()
                    .unwrap()
                    .entry(input.ctx.run_id.clone())
                    .or_insert_with(|| input.event.data.message.clone());

                step.sleep("hold-singleton", Duration::from_millis(1500))?;

                Ok(json!({ "message": input.event.data.message }))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![func.into()]).await;
    app.sync().await;

    client
        .send_event(&Event::new(
            &event_name,
            KeyedEventData {
                key: "group-1".to_string(),
                message: "first".to_string(),
            },
        ))
        .await
        .expect("first singleton event should send successfully");

    let started = wait_for_value(&started_runs, Duration::from_secs(5), |runs| !runs.is_empty()).await;
    let first_run_id = started
        .keys()
        .next()
        .cloned()
        .expect("first singleton run should exist");

    client
        .send_event(&Event::new(
            &event_name,
            KeyedEventData {
                key: "group-1".to_string(),
                message: "second".to_string(),
            },
        ))
        .await
        .expect("second singleton event should send successfully");

    wait_for_run_status(&first_run_id, "Completed", Duration::from_secs(15)).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let started = started_runs.lock().unwrap().clone();
    assert_eq!(started.len(), 1);
    assert_eq!(
        started.values().cloned().collect::<Vec<_>>(),
        vec!["first".to_string()]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_stops_an_in_flight_run_when_a_matching_event_arrives() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("function-config-cancel-app");
    let start_event_name = e2e_support::unique_name("test.function.cancel.start");
    let cancel_event_name = e2e_support::unique_name("test.function.cancel.signal");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let run_id_state = Arc::new(Mutex::new(None::<String>));
    let completed_after_sleep = Arc::new(AtomicUsize::new(0));

    let run_id_capture = Arc::clone(&run_id_state);
    let completed_after_sleep_capture = Arc::clone(&completed_after_sleep);
    let func: ServableFn<KeyedEventData, Error> = client.create_function(
        FunctionOpts::new("cancel-parent")
            .name("Cancel Parent")
            .cancel(
                FunctionCancel::new(&cancel_event_name)
                    .if_exp("event.data.key == async.data.key")
                    .timeout(Duration::from_secs(5)),
            ),
        Trigger::event(&start_event_name),
        move |input: Input<KeyedEventData>, step: StepTool| {
            let run_id_capture = Arc::clone(&run_id_capture);
            let completed_after_sleep_capture = Arc::clone(&completed_after_sleep_capture);

            async move {
                if run_id_capture.lock().unwrap().is_none() {
                    *run_id_capture.lock().unwrap() = Some(input.ctx.run_id.clone());
                }

                step.sleep("hold-cancel", Duration::from_secs(5))?;
                completed_after_sleep_capture.fetch_add(1, Ordering::SeqCst);

                Ok(json!({ "message": input.event.data.message }))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![func.into()]).await;
    app.sync().await;

    client
        .send_event(&Event::new(
            &start_event_name,
            KeyedEventData {
                key: "group-1".to_string(),
                message: "first".to_string(),
            },
        ))
        .await
        .expect("start event should send successfully");

    let run_id = wait_for_state(&run_id_state, Duration::from_secs(5)).await;
    wait_for_run_status(&run_id, "Running", Duration::from_secs(5)).await;

    client
        .send_event(&Event::new(
            &cancel_event_name,
            KeyedEventData {
                key: "group-1".to_string(),
                message: "cancel".to_string(),
            },
        ))
        .await
        .expect("cancel event should send successfully");

    let run = wait_for_run_status_matching(&run_id, Duration::from_secs(15), |status| {
        let normalized = status.to_ascii_lowercase();
        normalized.contains("cancel")
    })
    .await;

    assert!(run.status.to_ascii_lowercase().contains("cancel"));
    assert_eq!(completed_after_sleep.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrency_limit_serializes_step_execution_across_runs() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("function-config-concurrency-app");
    let event_name = e2e_support::unique_name("test.function.concurrency");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let step_starts = Arc::new(Mutex::new(HashMap::<String, Instant>::new()));
    let step_ends = Arc::new(Mutex::new(HashMap::<String, Instant>::new()));

    let step_starts_capture = Arc::clone(&step_starts);
    let step_ends_capture = Arc::clone(&step_ends);
    let func: ServableFn<KeyedEventData, Error> = client.create_function(
        FunctionOpts::new("concurrency-parent")
            .name("Concurrency Parent")
            .concurrency(FunctionConcurrency::limit(1)),
        Trigger::event(&event_name),
        move |input: Input<KeyedEventData>, step: StepTool| {
            let step_starts_capture = Arc::clone(&step_starts_capture);
            let step_ends_capture = Arc::clone(&step_ends_capture);

            async move {
                let message = input.event.data.message.clone();
                let step_result: String = step
                    .run("blocking-step", {
                        let message = message.clone();
                        move || {
                            let step_starts_capture = Arc::clone(&step_starts_capture);
                            let step_ends_capture = Arc::clone(&step_ends_capture);
                            let message = message.clone();

                            async move {
                                step_starts_capture
                                    .lock()
                                    .unwrap()
                                    .entry(message.clone())
                                    .or_insert_with(Instant::now);
                                tokio::time::sleep(Duration::from_millis(1500)).await;
                                step_ends_capture
                                    .lock()
                                    .unwrap()
                                    .entry(message.clone())
                                    .or_insert_with(Instant::now);

                                Ok::<_, E2eTestError>(message.clone())
                            }
                        }
                    })
                    .await?;

                Ok(json!({ "message": step_result }))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![func.into()]).await;
    app.sync().await;

    client
        .send_event(&Event::new(
            &event_name,
            KeyedEventData {
                key: "first".to_string(),
                message: "first".to_string(),
            },
        ))
        .await
        .expect("first concurrency event should send successfully");

    wait_for_value(&step_starts, Duration::from_secs(10), |starts| starts.contains_key("first"))
        .await;

    client
        .send_event(&Event::new(
            &event_name,
            KeyedEventData {
                key: "second".to_string(),
                message: "second".to_string(),
            },
        ))
        .await
        .expect("second concurrency event should send successfully");

    let starts = wait_for_value(&step_starts, Duration::from_secs(20), |starts| {
        starts.contains_key("first") && starts.contains_key("second")
    })
    .await;
    let ends = wait_for_value(&step_ends, Duration::from_secs(20), |ends| {
        ends.contains_key("first") && ends.contains_key("second")
    })
    .await;

    let first_end = ends["first"];
    let second_start = starts["second"];
    assert!(
        second_start >= first_end,
        "second step started before the first step finished"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn throttle_queues_runs_instead_of_dropping_them() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("function-config-throttle-app");
    let event_name = e2e_support::unique_name("test.function.throttle");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let run_starts = Arc::new(Mutex::new(HashMap::<String, Instant>::new()));

    let run_starts_capture = Arc::clone(&run_starts);
    let func: ServableFn<KeyedEventData, Error> = client.create_function(
        FunctionOpts::new("throttle-parent")
            .name("Throttle Parent")
            .throttle(FunctionThrottle::new(1, Duration::from_secs(2))),
        Trigger::event(&event_name),
        move |input: Input<KeyedEventData>, _step: StepTool| {
            let run_starts_capture = Arc::clone(&run_starts_capture);

            async move {
                run_starts_capture
                    .lock()
                    .unwrap()
                    .entry(input.event.data.message.clone())
                    .or_insert_with(Instant::now);

                Ok(json!({ "message": input.event.data.message }))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![func.into()]).await;
    app.sync().await;

    client
        .send_event(&Event::new(
            &event_name,
            KeyedEventData {
                key: "first".to_string(),
                message: "first".to_string(),
            },
        ))
        .await
        .expect("first throttled event should send successfully");

    wait_for_value(&run_starts, Duration::from_secs(10), |starts| starts.contains_key("first"))
        .await;

    client
        .send_event(&Event::new(
            &event_name,
            KeyedEventData {
                key: "second".to_string(),
                message: "second".to_string(),
            },
        ))
        .await
        .expect("second throttled event should send successfully");

    let starts = wait_for_value(&run_starts, Duration::from_secs(20), |starts| {
        starts.contains_key("first") && starts.contains_key("second")
    })
    .await;

    assert!(
        starts["second"].duration_since(starts["first"]) >= Duration::from_secs(1),
        "throttled second run started too quickly"
    );
}

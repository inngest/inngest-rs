mod e2e_support;

use e2e_support::{spawn_app, wait_for_run_status, wait_for_state, DevServer, DevServerLock};
use inngest::{
    client::Inngest,
    event::Event,
    function::{FunctionOpts, Input, ServableFn, Trigger},
    result::{DevError, Error},
    step_tool::Step as StepTool,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fmt::{Display, Formatter},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Debug, Deserialize, Serialize)]
struct EmptyEventData {}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct MemoizedStepOutput {
    message: String,
    from_step: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct MemoizedStepFailure {
    message: String,
}

impl Display for MemoizedStepFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for MemoizedStepFailure {}

impl From<MemoizedStepFailure> for Error {
    fn from(err: MemoizedStepFailure) -> Self {
        Error::Dev(DevError::Basic(err.message))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn step_run_reuses_the_memoized_result_after_replay() {
    // The dev server uses fixed ports, so each e2e test holds the suite-wide lock.
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("step-run-e2e-app");
    let event_name = e2e_support::unique_name("test.step-run.parent");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let run_id_state = Arc::new(Mutex::new(None::<String>));
    let step_result_state = Arc::new(Mutex::new(None::<MemoizedStepOutput>));
    let handler_invocations = Arc::new(AtomicUsize::new(0));
    let step_invocations = Arc::new(AtomicUsize::new(0));

    let run_id_capture = Arc::clone(&run_id_state);
    let step_result_capture = Arc::clone(&step_result_state);
    let handler_invocations_capture = Arc::clone(&handler_invocations);
    let step_invocations_capture = Arc::clone(&step_invocations);
    let step_run_fn: ServableFn<EmptyEventData, Error> = client.create_function(
        FunctionOpts::new("step-run-fn").name("Step Run Fn"),
        Trigger::event(&event_name),
        move |input: Input<EmptyEventData>, step: StepTool| {
            let run_id_capture = Arc::clone(&run_id_capture);
            let step_result_capture = Arc::clone(&step_result_capture);
            let handler_invocations_capture = Arc::clone(&handler_invocations_capture);
            let step_invocations_capture = Arc::clone(&step_invocations_capture);

            async move {
                // The whole function replays after the first step interruption.
                handler_invocations_capture.fetch_add(1, Ordering::SeqCst);
                *run_id_capture.lock().unwrap() = Some(input.ctx.run_id.clone());

                let step_result: MemoizedStepOutput = step
                    .run("memoized-step", || {
                        let step_invocations_capture = Arc::clone(&step_invocations_capture);

                        async move {
                            // The step body itself should execute only once.
                            step_invocations_capture.fetch_add(1, Ordering::SeqCst);

                            Ok::<_, MemoizedStepFailure>(MemoizedStepOutput {
                                message: "hello-from-step".to_string(),
                                from_step: true,
                            })
                        }
                    })
                    .await?;

                *step_result_capture.lock().unwrap() = Some(step_result.clone());

                Ok(json!(step_result))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![step_run_fn.into()]).await;
    app.sync().await;

    // Trigger the function through the dev server so the real replay path is exercised.
    client
        .send_event(&Event::new(&event_name, EmptyEventData {}))
        .await
        .expect("step-run event should send successfully");

    let run_id = wait_for_state(&run_id_state, Duration::from_secs(5)).await;
    let run = wait_for_run_status(&run_id, "Completed", Duration::from_secs(10)).await;
    let step_result = wait_for_state(&step_result_state, Duration::from_secs(5)).await;

    let expected = MemoizedStepOutput {
        message: "hello-from-step".to_string(),
        from_step: true,
    };

    // The handler replays, but the memoized step body should not run twice.
    assert_eq!(handler_invocations.load(Ordering::SeqCst), 2);
    assert_eq!(step_invocations.load(Ordering::SeqCst), 1);
    assert_eq!(step_result, expected);
    assert_eq!(run.output, json!(expected));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_step_ids_replay_with_distinct_memoized_results() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("step-run-repeat-app");
    let event_name = e2e_support::unique_name("test.step-run.repeat");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let run_id_state = Arc::new(Mutex::new(None::<String>));
    let repeated_results_state = Arc::new(Mutex::new(None::<Vec<String>>));
    let handler_invocations = Arc::new(AtomicUsize::new(0));
    let step_invocations = Arc::new(AtomicUsize::new(0));

    let run_id_capture = Arc::clone(&run_id_state);
    let repeated_results_capture = Arc::clone(&repeated_results_state);
    let handler_invocations_capture = Arc::clone(&handler_invocations);
    let step_invocations_capture = Arc::clone(&step_invocations);
    let step_run_fn: ServableFn<EmptyEventData, Error> = client.create_function(
        FunctionOpts::new("step-run-repeat-fn").name("Step Run Repeat Fn"),
        Trigger::event(&event_name),
        move |input: Input<EmptyEventData>, step: StepTool| {
            let run_id_capture = Arc::clone(&run_id_capture);
            let repeated_results_capture = Arc::clone(&repeated_results_capture);
            let handler_invocations_capture = Arc::clone(&handler_invocations_capture);
            let step_invocations_capture = Arc::clone(&step_invocations_capture);

            async move {
                handler_invocations_capture.fetch_add(1, Ordering::SeqCst);
                *run_id_capture.lock().unwrap() = Some(input.ctx.run_id.clone());

                let first: String = step
                    .run("same-id", || {
                        let step_invocations_capture = Arc::clone(&step_invocations_capture);
                        async move {
                            let call = step_invocations_capture.fetch_add(1, Ordering::SeqCst);
                            Ok::<_, MemoizedStepFailure>(format!("value-{}", call + 1))
                        }
                    })
                    .await?;

                let second: String = step
                    .run("same-id", || {
                        let step_invocations_capture = Arc::clone(&step_invocations_capture);
                        async move {
                            let call = step_invocations_capture.fetch_add(1, Ordering::SeqCst);
                            Ok::<_, MemoizedStepFailure>(format!("value-{}", call + 1))
                        }
                    })
                    .await?;

                let results = vec![first, second];
                *repeated_results_capture.lock().unwrap() = Some(results.clone());

                Ok(json!(results))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![step_run_fn.into()]).await;
    app.sync().await;

    client
        .send_event(&Event::new(&event_name, EmptyEventData {}))
        .await
        .expect("repeat-step event should send successfully");

    let run_id = wait_for_state(&run_id_state, Duration::from_secs(5)).await;
    let run = wait_for_run_status(&run_id, "Completed", Duration::from_secs(10)).await;
    let repeated_results = wait_for_state(&repeated_results_state, Duration::from_secs(5)).await;

    assert_eq!(handler_invocations.load(Ordering::SeqCst), 3);
    assert_eq!(step_invocations.load(Ordering::SeqCst), 2);
    assert_eq!(
        repeated_results,
        vec!["value-1".to_string(), "value-2".to_string()]
    );
    assert_eq!(run.output, json!(["value-1", "value-2"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memoized_step_errors_replay_without_rerunning_the_failed_step_body() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("step-run-error-app");
    let event_name = e2e_support::unique_name("test.step-run.error");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let run_id_state = Arc::new(Mutex::new(None::<String>));
    let step_error_state = Arc::new(Mutex::new(None::<String>));
    let handler_invocations = Arc::new(AtomicUsize::new(0));
    let step_invocations = Arc::new(AtomicUsize::new(0));

    let run_id_capture = Arc::clone(&run_id_state);
    let step_error_capture = Arc::clone(&step_error_state);
    let handler_invocations_capture = Arc::clone(&handler_invocations);
    let step_invocations_capture = Arc::clone(&step_invocations);
    let step_run_fn: ServableFn<EmptyEventData, Error> = client.create_function(
        FunctionOpts::new("step-run-error-fn")
            .name("Step Run Error Fn")
            .retries(0),
        Trigger::event(&event_name),
        move |input: Input<EmptyEventData>, step: StepTool| {
            let run_id_capture = Arc::clone(&run_id_capture);
            let step_error_capture = Arc::clone(&step_error_capture);
            let handler_invocations_capture = Arc::clone(&handler_invocations_capture);
            let step_invocations_capture = Arc::clone(&step_invocations_capture);

            async move {
                handler_invocations_capture.fetch_add(1, Ordering::SeqCst);
                *run_id_capture.lock().unwrap() = Some(input.ctx.run_id.clone());

                match step
                    .run::<MemoizedStepOutput, MemoizedStepFailure, _>("failing-step", || {
                        let step_invocations_capture = Arc::clone(&step_invocations_capture);

                        async move {
                            step_invocations_capture.fetch_add(1, Ordering::SeqCst);
                            Err::<MemoizedStepOutput, MemoizedStepFailure>(MemoizedStepFailure {
                                message: "step exploded".to_string(),
                            })
                        }
                    })
                    .await
                {
                    Ok(value) => Ok(json!(value)),
                    Err(Error::Dev(DevError::Step(err))) => {
                        *step_error_capture.lock().unwrap() = Some(err.message.clone());
                        Ok(json!({ "step_error": err.message }))
                    }
                    Err(err) => Err(err),
                }
            }
        },
    );

    let app = spawn_app(client.clone(), vec![step_run_fn.into()]).await;
    app.sync().await;

    client
        .send_event(&Event::new(&event_name, EmptyEventData {}))
        .await
        .expect("step-error event should send successfully");

    let run_id = wait_for_state(&run_id_state, Duration::from_secs(5)).await;
    let run = wait_for_run_status(&run_id, "Completed", Duration::from_secs(10)).await;
    let step_error = wait_for_state(&step_error_state, Duration::from_secs(5)).await;

    assert_eq!(handler_invocations.load(Ordering::SeqCst), 2);
    assert_eq!(step_invocations.load(Ordering::SeqCst), 1);
    assert_eq!(step_error, "step exploded".to_string());
    assert_eq!(run.output, json!({ "step_error": "step exploded" }));
}

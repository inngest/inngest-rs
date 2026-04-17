mod e2e_support;

use e2e_support::{spawn_app, wait_for_run_status, wait_for_state, DevServer, DevServerLock};
use inngest::{
    client::Inngest,
    event::Event,
    function::{FunctionOpts, Input, ServableFn, Trigger},
    result::Error,
    step_tool::Step as StepTool,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Debug, Deserialize, Serialize)]
struct EmptyEventData {}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn step_sleep_pauses_and_resumes_the_run() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("sleep-e2e-app");
    let sleep_event_name = e2e_support::unique_name("test.sleep.parent");
    let sleep_duration = Duration::from_millis(1200);

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let run_id_state = Arc::new(Mutex::new(None::<String>));
    let resumed_at_state = Arc::new(Mutex::new(None::<Instant>));
    let handler_invocations = Arc::new(AtomicUsize::new(0));

    let run_id_capture = Arc::clone(&run_id_state);
    let resumed_at_capture = Arc::clone(&resumed_at_state);
    let handler_invocations_capture = Arc::clone(&handler_invocations);
    let sleep_fn: ServableFn<EmptyEventData, Error> = client.create_function(
        FunctionOpts::new("sleep-fn").name("Sleep Fn"),
        Trigger::event(&sleep_event_name),
        move |input: Input<EmptyEventData>, step: StepTool| {
            let run_id_capture = Arc::clone(&run_id_capture);
            let resumed_at_capture = Arc::clone(&resumed_at_capture);
            let handler_invocations_capture = Arc::clone(&handler_invocations_capture);

            async move {
                // The function body is replayed after the sleep completes, so count top-level calls.
                handler_invocations_capture.fetch_add(1, Ordering::SeqCst);
                *run_id_capture.lock().unwrap() = Some(input.ctx.run_id.clone());

                // The first pass interrupts here; the second pass resumes and continues below.
                step.sleep("pause-before-complete", sleep_duration)?;

                *resumed_at_capture.lock().unwrap() = Some(Instant::now());

                Ok(json!({ "slept": true }))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![sleep_fn.into()]).await;
    app.sync().await;

    let sent_at = Instant::now();
    client
        .send_event(&Event::new(&sleep_event_name, EmptyEventData {}))
        .await
        .expect("sleep event should send successfully");

    let run_id = wait_for_state(&run_id_state, Duration::from_secs(5)).await;

    // A sleeping run should remain active before the scheduled resume fires.
    wait_for_run_status(&run_id, "Running", Duration::from_secs(5)).await;

    let run = wait_for_run_status(&run_id, "Completed", Duration::from_secs(10)).await;
    let resumed_at = wait_for_state(&resumed_at_state, Duration::from_secs(5)).await;

    assert!(
        resumed_at.duration_since(sent_at) >= Duration::from_secs(1),
        "sleep resumed too quickly"
    );
    assert_eq!(handler_invocations.load(Ordering::SeqCst), 2);
    assert_eq!(run.output, json!({ "slept": true }));
}

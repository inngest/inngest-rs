mod e2e_support;

use e2e_support::{spawn_app, wait_for_run_status, wait_for_state, DevServer, DevServerLock};
use inngest::{
    client::Inngest,
    event::Event,
    function::{FunctionOpts, Input, ServableFn, Trigger},
    result::Error,
    step_tool::{Step as StepTool, WaitForEventOpts},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Debug, Default)]
struct WaitForEventMatch {
    id: String,
    value: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct WaitForEventData {
    value: u32,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wait_for_event_returns_the_fulfilled_event() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("wait-match-app");
    let base_event_name = e2e_support::unique_name("test.wait.match");
    let fulfill_event_name = format!("{base_event_name}.fulfilled");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let run_id_state = Arc::new(Mutex::new(None::<String>));
    let waited_event_state = Arc::new(Mutex::new(None::<WaitForEventMatch>));

    let run_id_capture = Arc::clone(&run_id_state);
    let waited_event_capture = Arc::clone(&waited_event_state);
    let fulfill_event_name_for_wait = fulfill_event_name.clone();
    let wait_fn: ServableFn<WaitForEventData, Error> = client.create_function(
        FunctionOpts::new("wait-for-event-match-fn").name("Wait For Event Match Fn"),
        Trigger::event(&base_event_name),
        move |input: Input<WaitForEventData>, step: StepTool| {
            let run_id_capture = Arc::clone(&run_id_capture);
            let waited_event_capture = Arc::clone(&waited_event_capture);
            let fulfill_event_name = fulfill_event_name_for_wait.clone();

            async move {
                *run_id_capture.lock().unwrap() = Some(input.ctx.run_id.clone());

                let waited_event = step.wait_for_event::<WaitForEventData>(
                    "wait-for-match",
                    WaitForEventOpts {
                        event: fulfill_event_name,
                        timeout: Duration::from_secs(10),
                        if_exp: Some("event.data.value == async.data.value".to_string()),
                    },
                )?;

                if let Some(waited_event) = waited_event {
                    *waited_event_capture.lock().unwrap() = Some(WaitForEventMatch {
                        id: waited_event.id.unwrap_or_default(),
                        value: waited_event.data.value,
                    });

                    Ok(json!("fulfilled"))
                } else {
                    Ok(json!("empty"))
                }
            }
        },
    );

    let app = spawn_app(client.clone(), vec![wait_fn.into()]).await;
    app.sync().await;

    client
        .send_event(&Event::new(
            &base_event_name,
            WaitForEventData { value: 42 },
        ))
        .await
        .expect("base event should send successfully");

    let run_id = wait_for_state(&run_id_state, Duration::from_secs(5)).await;
    wait_for_run_status(&run_id, "Running", Duration::from_secs(10)).await;

    client
        .send_event(&Event::new(
            &fulfill_event_name,
            WaitForEventData { value: 42 },
        ))
        .await
        .expect("fulfillment event should send successfully");

    let run = wait_for_run_status(&run_id, "Completed", Duration::from_secs(10)).await;
    let waited_event = wait_for_state(&waited_event_state, Duration::from_secs(10)).await;

    assert!(!waited_event.id.is_empty());
    assert_eq!(waited_event.value, 42);
    assert_eq!(run.output, json!("fulfilled"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wait_for_event_times_out_without_a_match() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("wait-timeout-app");
    let base_event_name = e2e_support::unique_name("test.wait.timeout");
    let missing_event_name = format!("{base_event_name}.never");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let run_id_state = Arc::new(Mutex::new(None::<String>));
    let waited_event_state = Arc::new(Mutex::new(None::<WaitForEventMatch>));

    let run_id_capture = Arc::clone(&run_id_state);
    let waited_event_capture = Arc::clone(&waited_event_state);
    let wait_fn: ServableFn<WaitForEventData, Error> = client.create_function(
        FunctionOpts::new("wait-for-event-timeout-fn").name("Wait For Event Timeout Fn"),
        Trigger::event(&base_event_name),
        move |input: Input<WaitForEventData>, step: StepTool| {
            let run_id_capture = Arc::clone(&run_id_capture);
            let waited_event_capture = Arc::clone(&waited_event_capture);
            let missing_event_name = missing_event_name.clone();

            async move {
                *run_id_capture.lock().unwrap() = Some(input.ctx.run_id.clone());

                let waited_event = step.wait_for_event::<WaitForEventData>(
                    "wait-for-timeout",
                    WaitForEventOpts {
                        event: missing_event_name,
                        timeout: Duration::from_secs(1),
                        if_exp: None,
                    },
                )?;

                if let Some(waited_event) = waited_event {
                    *waited_event_capture.lock().unwrap() = Some(WaitForEventMatch {
                        id: waited_event.id.unwrap_or_default(),
                        value: waited_event.data.value,
                    });
                    Ok(json!("fulfilled"))
                } else {
                    Ok(json!("empty"))
                }
            }
        },
    );

    let app = spawn_app(client.clone(), vec![wait_fn.into()]).await;
    app.sync().await;

    client
        .send_event(&Event::new(&base_event_name, WaitForEventData { value: 7 }))
        .await
        .expect("base event should send successfully");

    let run_id = wait_for_state(&run_id_state, Duration::from_secs(5)).await;
    let run = wait_for_run_status(&run_id, "Completed", Duration::from_secs(10)).await;

    assert!(waited_event_state.lock().unwrap().is_none());
    assert_eq!(run.output, json!("empty"));
}

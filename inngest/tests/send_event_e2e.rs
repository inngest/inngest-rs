mod e2e_support;

use e2e_support::{
    spawn_app, wait_for_event_runs, wait_for_run_status, wait_for_state, DevServer, DevServerLock,
};
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
    time::Duration,
};

#[derive(Clone, Debug, Default)]
struct ReceivedChildEvent {
    id: String,
    name: String,
    message: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct EmptyEventData {}

#[derive(Debug, Deserialize, Serialize)]
struct SendChildEventData {
    message: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn step_send_event_triggers_child_function() {
    // The dev server uses fixed ports, so each e2e test holds the suite-wide lock.
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("send-e2e-app");
    let parent_event_name = e2e_support::unique_name("test.send.parent");
    let child_event_name = e2e_support::unique_name("test.send.child");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let parent_run_id = Arc::new(Mutex::new(None::<String>));
    let sent_event_ids = Arc::new(Mutex::new(None::<Vec<String>>));
    let child_event = Arc::new(Mutex::new(None::<ReceivedChildEvent>));

    let child_state = Arc::clone(&child_event);
    let child_fn: ServableFn<SendChildEventData, Error> = client.create_function(
        FunctionOpts::new("send-child-fn").name("Send Child Fn"),
        Trigger::event(&child_event_name),
        move |input: Input<SendChildEventData>, _step: StepTool| {
            let child_state = Arc::clone(&child_state);

            async move {
                *child_state.lock().unwrap() = Some(ReceivedChildEvent {
                    id: input.event.id.unwrap_or_default(),
                    name: input.event.name,
                    message: input.event.data.message,
                });

                Ok(json!("child-complete"))
            }
        },
    );

    let parent_run_state = Arc::clone(&parent_run_id);
    let sent_event_ids_state = Arc::clone(&sent_event_ids);
    let child_event_name_for_parent = child_event_name.clone();
    let parent_fn: ServableFn<EmptyEventData, Error> = client.create_function(
        FunctionOpts::new("send-parent-fn").name("Send Parent Fn"),
        Trigger::event(&parent_event_name),
        move |input: Input<EmptyEventData>, step: StepTool| {
            let parent_run_state = Arc::clone(&parent_run_state);
            let sent_event_ids_state = Arc::clone(&sent_event_ids_state);
            let child_event_name = child_event_name_for_parent.clone();

            async move {
                // Capture the parent run so the harness can poll the dev server for completion.
                *parent_run_state.lock().unwrap() = Some(input.ctx.run_id.clone());

                // `send_event` is durable, so the parent run is interrupted and replayed.
                let ids = step
                    .send_event(
                        "send-child",
                        Event::new(
                            &child_event_name,
                            SendChildEventData {
                                message: "hello-from-parent".to_string(),
                            },
                        ),
                    )
                    .await?;

                *sent_event_ids_state.lock().unwrap() = Some(ids.clone());

                Ok(json!({ "event_ids": ids }))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![child_fn.into(), parent_fn.into()]).await;
    app.sync().await;

    // Send the parent event through the dev server so the full SDK handshake is exercised.
    client
        .send_event(&Event::new(&parent_event_name, EmptyEventData {}))
        .await
        .expect("parent event should send successfully");

    let run_id = wait_for_state(&parent_run_id, Duration::from_secs(5)).await;
    wait_for_run_status(&run_id, "Completed", Duration::from_secs(10)).await;

    let event_ids = wait_for_state(&sent_event_ids, Duration::from_secs(5)).await;
    assert_eq!(event_ids.len(), 1);

    // The emitted child event should create its own completed run in the dev server.
    let child_runs = wait_for_event_runs(&event_ids[0], Duration::from_secs(10)).await;
    assert!(!child_runs[0].run_id.is_empty());
    assert_eq!(child_runs[0].status, "Completed");

    // The child function should observe the durable event payload produced by the parent.
    let received = wait_for_state(&child_event, Duration::from_secs(10)).await;
    assert_eq!(received.id, event_ids[0]);
    assert_eq!(received.name, child_event_name);
    assert_eq!(received.message, "hello-from-parent");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn step_send_events_replays_and_returns_all_emitted_ids() {
    let _lock = DevServerLock::acquire();
    let _dev_server = DevServer::start().await;

    let app_name = e2e_support::unique_name("send-many-e2e-app");
    let parent_event_name = e2e_support::unique_name("test.send.many.parent");
    let child_event_name = e2e_support::unique_name("test.send.many.child");

    let client = Inngest::new(&app_name).dev(e2e_support::DEV_SERVER_ORIGIN);
    let parent_run_id = Arc::new(Mutex::new(None::<String>));
    let sent_event_ids = Arc::new(Mutex::new(None::<Vec<String>>));
    let child_events = Arc::new(Mutex::new(Vec::<ReceivedChildEvent>::new()));
    let parent_invocations = Arc::new(AtomicUsize::new(0));

    let child_events_state = Arc::clone(&child_events);
    let child_fn: ServableFn<SendChildEventData, Error> = client.create_function(
        FunctionOpts::new("send-many-child-fn").name("Send Many Child Fn"),
        Trigger::event(&child_event_name),
        move |input: Input<SendChildEventData>, _step: StepTool| {
            let child_events_state = Arc::clone(&child_events_state);

            async move {
                child_events_state.lock().unwrap().push(ReceivedChildEvent {
                    id: input.event.id.unwrap_or_default(),
                    name: input.event.name,
                    message: input.event.data.message,
                });

                Ok(json!("child-complete"))
            }
        },
    );

    let parent_run_state = Arc::clone(&parent_run_id);
    let sent_event_ids_state = Arc::clone(&sent_event_ids);
    let parent_invocations_state = Arc::clone(&parent_invocations);
    let child_event_name_for_parent = child_event_name.clone();
    let parent_fn: ServableFn<EmptyEventData, Error> = client.create_function(
        FunctionOpts::new("send-many-parent-fn").name("Send Many Parent Fn"),
        Trigger::event(&parent_event_name),
        move |input: Input<EmptyEventData>, step: StepTool| {
            let parent_run_state = Arc::clone(&parent_run_state);
            let sent_event_ids_state = Arc::clone(&sent_event_ids_state);
            let parent_invocations_state = Arc::clone(&parent_invocations_state);
            let child_event_name = child_event_name_for_parent.clone();

            async move {
                parent_invocations_state.fetch_add(1, Ordering::SeqCst);
                *parent_run_state.lock().unwrap() = Some(input.ctx.run_id.clone());

                let ids = step
                    .send_events(
                        "send-children",
                        vec![
                            Event::new(
                                &child_event_name,
                                SendChildEventData {
                                    message: "hello-from-parent-1".to_string(),
                                },
                            ),
                            Event::new(
                                &child_event_name,
                                SendChildEventData {
                                    message: "hello-from-parent-2".to_string(),
                                },
                            ),
                        ],
                    )
                    .await?;

                *sent_event_ids_state.lock().unwrap() = Some(ids.clone());

                Ok(json!({ "event_ids": ids }))
            }
        },
    );

    let app = spawn_app(client.clone(), vec![child_fn.into(), parent_fn.into()]).await;
    app.sync().await;

    client
        .send_event(&Event::new(&parent_event_name, EmptyEventData {}))
        .await
        .expect("parent multi-send event should send successfully");

    let run_id = wait_for_state(&parent_run_id, Duration::from_secs(5)).await;
    let run = wait_for_run_status(&run_id, "Completed", Duration::from_secs(10)).await;
    let event_ids = wait_for_state(&sent_event_ids, Duration::from_secs(5)).await;

    assert_eq!(parent_invocations.load(Ordering::SeqCst), 2);
    assert_eq!(event_ids.len(), 2);
    assert_eq!(run.output, json!({ "event_ids": event_ids.clone() }));

    for event_id in &event_ids {
        let child_runs = wait_for_event_runs(event_id, Duration::from_secs(10)).await;
        assert!(!child_runs[0].run_id.is_empty());
        assert_eq!(child_runs[0].status, "Completed");
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let received = child_events.lock().unwrap().clone();
        if received.len() == 2 {
            let ids = received.iter().map(|event| event.id.clone()).collect::<Vec<_>>();
            let messages = received
                .iter()
                .map(|event| event.message.clone())
                .collect::<Vec<_>>();
            assert_eq!(ids.len(), 2);
            assert!(messages.contains(&"hello-from-parent-1".to_string()));
            assert!(messages.contains(&"hello-from-parent-2".to_string()));
            break;
        }

        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for both child events"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

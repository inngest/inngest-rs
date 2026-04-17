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
    sync::{Arc, Mutex},
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

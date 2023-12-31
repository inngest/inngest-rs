use inngest::event::{send_event, send_events, Event};
use inngest_macros::InngestEvent;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Data {
    foo: u8,
    bar: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, InngestEvent)]
struct TestEvent {
    name: String,
    data: Data,
}

#[tokio::main]
async fn main() {
    let evt = TestEvent {
        name: "test/event".to_string(),
        data: Data { foo: 1, bar: 2 },
    };

    let evt2 = TestEvent {
        name: "test/yolo".to_string(),
        data: Data { foo: 10, bar: 20 },
    };

    let evts: Vec<&dyn Event> = vec![&evt, &evt2];

    match send_event(&evt).await {
        Ok(_) => println!("Success"),
        Err(_) => println!("Error"),
    }

    match send_events(evts.as_slice()).await {
        Ok(_) => println!("List success"),
        Err(_) => println!("List error"),
    }
}

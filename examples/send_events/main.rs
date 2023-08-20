use std::any::Any;

use inngest::event::{send_event, send_events, Event};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
struct Data {
    foo: u8,
    bar: u8,
}

#[derive(Serialize, Deserialize, Clone)]
struct TestEvent {
    name: String,
    data: Data,
}

#[typetag::serde]
impl Event for TestEvent {
    fn id(&self) -> Option<String> {
        None
    }

    fn name(&self) -> String {
        "test/event".to_string()
    }

    fn data(&self) -> &dyn Any {
        &self.data
    }

    fn user(&self) -> Option<&dyn Any> {
        None
    }

    fn timestamp(&self) -> Option<u64> {
        None
    }

    fn version(&self) -> Option<String> {
        None
    }
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

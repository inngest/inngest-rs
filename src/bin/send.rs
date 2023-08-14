use inngest::event::{send_event, send_events, Event};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct Data {
    foo: u8,
    bar: u8,
}

#[tokio::main]
async fn main() {
    let evt: Event<Data, ()> = Event {
        id: None,
        name: "test/event".to_string(),
        data: Data { foo: 1, bar: 2 },
        user: None,
        ts: 0,
    };

    let evts: Vec<Event<Data, ()>> = vec![
        evt.clone(),
        Event {
            id: None,
            name: "test/yolo".to_string(),
            data: Data { foo: 10, bar: 20 },
            user: None,
            ts: 0,
        },
    ];

    match send_event(evt).await {
        Ok(_) => println!("Success"),
        Err(_) => println!("Error"),
    }

    match send_events(&evts).await {
        Ok(_) => println!("List success"),
        Err(_) => println!("List error"),
    }
}

use inngest::event::{send_event, send_events, Event};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
struct Data {
    foo: u8,
    bar: u8,
}

#[tokio::main]
async fn main() {
    let evt: Event<Data, ()> = Event {
        name: "test/event".to_string(),
        data: Data { foo: 1, bar: 2 },
        ..Default::default()
    };

    let evts: Vec<Event<Data, ()>> = vec![
        evt.clone(),
        Event {
            name: "test/yolo".to_string(),
            data: Data { foo: 10, bar: 20 },
            ..Default::default()
        },
    ];

    match send_event(&evt).await {
        Ok(_) => println!("Success"),
        Err(_) => println!("Error"),
    }

    match send_events(&evts).await {
        Ok(_) => println!("List success"),
        Err(_) => println!("List error"),
    }
}

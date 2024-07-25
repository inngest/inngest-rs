use inngest::{event::Event, Inngest};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Data {
    foo: u8,
    bar: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TestEvent<T> {
    name: String,
    data: T,
}

#[tokio::main]
async fn main() {
    let client = Inngest::new("send-events").event_key("yolo");
    let evt = Event::<Data>::new("test/event", Data { foo: 1, bar: 2 });

    let evt2 = Event::<Data>::new("test/yolo", Data { foo: 10, bar: 20 });

    let evts: Vec<&Event<Data>> = vec![&evt, &evt2];

    match client.send_event(&evt).await {
        Ok(_) => println!("Success"),
        Err(_) => println!("Error"),
    }

    match client.send_events(evts.as_slice()).await {
        Ok(_) => println!("List success"),
        Err(_) => println!("List error"),
    }
}

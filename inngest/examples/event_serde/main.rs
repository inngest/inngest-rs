use inngest::event::Event;
use inngest_macros::InngestEvent;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, InngestEvent)]
struct DummyEvent {
    id: String,
    // TODO:
    #[event_name = "test/event"]
    name: String,
    data: DummyData,
}

#[derive(Serialize, Deserialize, Debug)]
struct DummyData {
    foo: String,
    bar: u8,
}

fn main() {
    let event = DummyEvent {
        id: "something".to_string(),
        name: "test/event".to_string(),
        data: DummyData {
            foo: "hello".to_string(),
            bar: 10,
        },
    };

    let jstr = json!(event);
    println!("JSON: {:?}", jstr);

    let jval = json!({
        "type": "DummyEvent",
        "value": {
            "data": {
                "foo": "Hello",
                "bar": 8
            },
            "id": "01H8DY42R2PVPJV162TEKJ7RYS",
            "name": "test/event",
            "ts": 1692684913410 as i64,
            "user": {},
        }
    });

    // println!("ID: {:?}", event.id());
    // println!("Name: {:?}", event.name());
    // println!("Data: {:?}", event.data());
    println!("JSON: {:?}", &jval);

    match serde_json::from_value::<Box<dyn Event>>(jval) {
        Ok(result) => println!("Result: {:?}", result),
        Err(err) => println!("Error: {:?}", err),
    };
}

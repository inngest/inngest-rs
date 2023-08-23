use inngest::event::Event;
use inngest_macros::InngestEvent;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, InngestEvent)]
struct DummyEvent {
    name: String,
    data: DummyData,
}

#[derive(Serialize, Deserialize, Debug)]
struct DummyData {
    foo: String,
    bar: u8,
}

// #[typetag::serde]
// impl Event for DummyEvent {
//     fn name(&self) -> String {
//         "test/event".to_string()
//     }

//     fn data(&self) -> &dyn std::any::Any {
//         &self.data
//     }
// }

fn main() {
    let event = DummyEvent {
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

    match serde_json::from_value::<Box<dyn Event>>(jval) {
        Ok(result) => println!("Result: {:?}", result),
        Err(err) => println!("Error: {:?}", err),
    };
}

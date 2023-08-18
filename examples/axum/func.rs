use inngest::{
    event::Event,
    function::{create_function, FunctionOps, Input, ServableFunction, Trigger},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct DummyData {
    pub yo: u8,
    pub lo: u8,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct User {}

pub fn dummy_fn() -> Box<dyn ServableFunction<T = Event<DummyData, User>> + Send + Sync + 'static> {
    create_function(
        FunctionOps {
            name: "Dummy func".to_string(),
            ..Default::default()
        },
        Trigger::EventTrigger {
            event: "test/event".to_string(),
            expression: None,
        },
        |_input: Input<Event<DummyData, User>>| {
            println!("In dummy function");

            Ok(Box::new(()))
        },
    )
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct HelloData {
    pub hello: String,
    pub world: String,
}

pub fn hello_fn() -> Box<dyn ServableFunction<T = Event<HelloData, User>> + Send + Sync + 'static> {
    create_function(
        FunctionOps {
            name: "Hello func".to_string(),
            ..Default::default()
        },
        Trigger::EventTrigger {
            event: "test/hello".to_string(),
            expression: None,
        },
        |_input: Input<Event<HelloData, User>>| {
            println!("In Hello function");

            Ok(Box::new(()))
        },
    )
}

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

pub fn dummy_fn() -> impl ServableFunction {
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

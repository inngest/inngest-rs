use inngest::{
    event::Event,
    function::{create_function, FunctionOps, Input, ServableFunction, Trigger},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct DummyData {
    pub yo: u8,
    pub lo: u8,
}

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
        |_input: Input<Event<DummyData, ()>>| {
            println!("In dummy function");

            Ok(Box::new(()))
        },
    )
}

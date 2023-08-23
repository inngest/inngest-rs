use serde::{Deserialize, Serialize};
use serde_json::json;

fn main() {
    // let something = Something::default();

    let jval = json!({
        "yo": "yo",
        "lo": "lo"
    });

    let something: Something = serde_json::from_value(jval).unwrap();
    println!("Something: {:#?}", something);
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct Something {
    yo: String,
    lo: String,
}

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub trait InngestEvent: Serialize + for<'a> Deserialize<'a> + Debug + 'static {}
impl<T: Serialize + for<'a> Deserialize<'a> + Debug + 'static> InngestEvent for T {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Event<T>
where
    T: 'static,
{
    pub id: Option<String>,
    pub name: String,
    pub data: T,
    #[serde(rename = "ts")]
    pub timestamp: Option<i64>,
    #[serde(rename = "v")]
    pub version: Option<String>,
}

impl<T> Event<T>
where
    T: InngestEvent,
{
    pub fn new(name: &str, data: T) -> Self {
        Event {
            id: None,
            name: name.to_string(),
            data,
            timestamp: None,
            version: None,
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    pub fn timestamp(mut self, ts: i64) -> Self {
        self.timestamp = Some(ts);
        self
    }

    pub fn version(mut self, v: &str) -> Self {
        self.version = Some(v.to_string());
        self
    }
}

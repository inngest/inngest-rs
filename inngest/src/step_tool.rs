use std::{collections::HashMap, time::Duration};

use base16;
use serde::Serialize;
use serde_json::json;
use sha1::{Digest, Sha1};

use crate::result::{Error, FlowControlError};

#[derive(Serialize)]
enum Opcode {
    StepRun,
    Sleep,
    WaitForEvent,
    InvokeFunction,
    // StepPlanned
}

#[derive(Serialize)]
pub(crate) struct GeneratorOpCode {
    op: Opcode,
    id: String,
    name: String,
    #[serde(rename(serialize = "displayName"))]
    display_name: String,
    data: serde_json::Value,
    opts: HashMap<String, String>,
}

pub struct Step {
    state: HashMap<String, Option<String>>,
    indices: HashMap<String, u64>,
    pub(crate) genop: Vec<GeneratorOpCode>,
}

impl Step {
    pub fn new(state: &HashMap<String, Option<String>>) -> Self {
        Step {
            state: state.clone(),
            indices: HashMap::new(),
            genop: vec![],
        }
    }

    // TODO: run

    pub fn sleep(&mut self, id: &str, dur: Duration) -> Result<(), Error> {
        let mut pos = 0;
        match self.indices.get_mut(id) {
            None => {
                self.indices.insert(id.to_string(), 0);
            }
            Some(v) => {
                *v = *v + 1;
                pos = *v;
            }
        }

        let op = UnhashedOp {
            id: id.to_string(),
            pos,
        };
        let hashed = op.hash();

        match self.state.get(&hashed) {
            // if state already exists, it means we already slept
            Some(_) => Ok(()),

            // TODO: if no state exists, we need to signal to sleep
            None => {
                let mut opts = HashMap::new();
                // TODO: convert `dur` to string format
                // ref: https://github.com/RonniSkansing/duration-string
                opts.insert("duration".to_string(), "10s".to_string());

                self.genop.push(GeneratorOpCode {
                    op: Opcode::Sleep,
                    id: hashed,
                    name: id.to_string(),
                    display_name: id.to_string(),
                    data: json!({}),
                    opts,
                });

                Err(Error::Interupt(FlowControlError::StepGenerator))
            }
        }
    }

    // TODO: sleep_until
    // TODO: wait_for_event
    // TODO: invoke
    // TODO: send_event
    // TODO: send_events
}

struct UnhashedOp {
    id: String,
    pos: u64,
    // TODO: need an opts as map??
}

impl UnhashedOp {
    fn hash(&self) -> String {
        let key = if self.pos > 0 {
            format!("{}:{}", self.id, self.pos)
        } else {
            self.id.to_string()
        };

        let mut hasher = Sha1::new();
        hasher.update(key.as_bytes());
        let res = hasher.finalize();

        base16::encode_upper(res.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_op_hash() {
        let op = UnhashedOp {
            id: "hello".to_string(),
            pos: 0,
        };

        assert_eq!(op.hash(), "AAF4C61DDCC5E8A2DABEDE0F3B482CD9AEA9434D");
    }

    #[test]
    fn test_op_hash_with_position() {
        let op = UnhashedOp {
            id: "hello".to_string(),
            pos: 1,
        };

        assert_eq!(op.hash(), "20A9BB9477C4AC565CF084D1614C58BBF0A523FF");
    }
}

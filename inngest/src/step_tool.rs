use std::{collections::HashMap, error::Error, time::Duration};

use base16;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};

use crate::result::{FlowControlError, InggestError, StepError};

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
    data: Option<serde_json::Value>,
    opts: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<StepError>,
}

pub struct Step {
    state: HashMap<String, Option<Value>>,
    indices: HashMap<String, u64>,
    pub(crate) genop: Vec<GeneratorOpCode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum StepRunResult<T, E> {
    Data(T),
    Error(E),
}

impl Step {
    pub fn new(state: &HashMap<String, Option<Value>>) -> Self {
        Step {
            state: state.clone(),
            indices: HashMap::new(),
            genop: vec![],
        }
    }

    pub fn run<T, F, E>(&mut self, id: &str, f: F) -> Result<T, InggestError>
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
        E: for<'a> Deserialize<'a> + Serialize + Error + Into<InggestError>,
        F: FnOnce() -> Result<T, E>,
    {
        let mut pos = 0;
        match self.indices.get_mut(id) {
            None => {
                self.indices.insert(id.to_string(), 0);
            }
            Some(v) => {
                *v += 1;
                pos = *v;
            }
        }
        let op = UnhashedOp {
            id: id.to_string(),
            pos,
        };
        let hashed = op.hash();

        if let Some(Some(stored_value)) = self.state.remove(&hashed) {
            let unwrapped: StepRunResult<T, E> = serde_json::from_value(stored_value)
                .map_err(|e| InggestError::Basic(e.to_string()))?;

            match unwrapped {
                StepRunResult::Data(data) => return Ok(data),
                StepRunResult::Error(err) => return Err(err.into()),
            }
        }

        // If we're here, we need to execute the function
        match f() {
            Ok(result) => {
                let serialized = serde_json::to_value(&result)
                    .map_err(|e| InggestError::Basic(e.to_string()))?;

                self.genop.push(GeneratorOpCode {
                    op: Opcode::StepRun,
                    id: hashed,
                    name: id.to_string(),
                    display_name: id.to_string(),
                    data: serialized.into(),
                    opts: HashMap::new(),
                    error: None,
                });
                Err(InggestError::Interrupt(FlowControlError::StepGenerator))
            }
            Err(err) => {
                let serialized_err =
                    serde_json::to_value(&err).map_err(|e| InggestError::Basic(e.to_string()))?;

                let error = StepError {
                    name: "Step failed".to_string(),
                    message: err.to_string(),
                    stack: None,
                    data: Some(serialized_err),
                };
                self.genop.push(GeneratorOpCode {
                    op: Opcode::StepRun,
                    id: hashed,
                    name: id.to_string(),
                    display_name: id.to_string(),
                    data: None,
                    opts: HashMap::new(),
                    error: Some(error),
                });
                Err(InggestError::Interrupt(FlowControlError::StepGenerator))
            }
        }
    }

    pub fn sleep(&mut self, id: &str, dur: Duration) -> Result<(), InggestError> {
        let mut pos = 0;
        match self.indices.get_mut(id) {
            None => {
                self.indices.insert(id.to_string(), 0);
            }
            Some(v) => {
                *v += 1;
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
                    data: None,
                    opts,
                    error: None,
                });

                Err(InggestError::Interrupt(FlowControlError::StepGenerator))
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

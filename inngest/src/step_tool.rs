use std::{
    collections::HashMap,
    error::Error as StdError,
    future::Future,
    time::{self, Duration, SystemTime},
};

use base16;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use state::State;

use crate::{
    basic_error,
    event::{Event, InngestEvent},
    result::{Error, FlowControlError, StepError},
    utils::duration,
};

#[derive(Serialize, Clone)]
enum Opcode {
    StepRun,
    Sleep,
    WaitForEvent,
    InvokeFunction,
    // StepPlanned
}

#[derive(Serialize, Clone)]
pub(crate) struct GeneratorOpCode {
    op: Opcode,
    id: String,
    name: String,
    #[serde(rename(serialize = "displayName"))]
    display_name: String,
    data: Option<serde_json::Value>,
    opts: Value,
}

mod state {
    use super::{GeneratorOpCode, Op};
    use crate::result::StepError;
    use serde_json::Value;
    use std::{
        collections::HashMap,
        sync::{Arc, RwLock},
    };

    /// Keep this private so that we can hide the mutex and prevent deadlocks
    struct InnerState {
        state: HashMap<String, Option<Value>>,
        indices: HashMap<String, u64>,
        genop: Vec<GeneratorOpCode>,
        error: Option<StepError>,
    }

    impl InnerState {
        fn new_op(&mut self, id: &str) -> Op {
            let pos = self.indices.entry(id.to_string()).or_insert(0);
            *pos += 1;
            Op {
                id: id.to_string(),
                pos: *pos,
            }
        }
    }

    #[derive(Clone)]
    pub struct State {
        inner: Arc<RwLock<InnerState>>,
    }

    impl State {
        pub fn new(state: &HashMap<String, Option<Value>>) -> Self {
            State {
                inner: Arc::new(RwLock::new(InnerState {
                    state: state.clone(),
                    indices: HashMap::new(),
                    genop: Vec::new(),
                    error: None,
                })),
            }
        }

        pub fn new_op(&self, id: &str) -> Op {
            self.inner.write().unwrap().new_op(id)
        }

        pub fn push_op(&self, op: GeneratorOpCode) {
            self.inner.write().unwrap().genop.push(op);
        }

        pub fn genop(&self) -> Vec<GeneratorOpCode> {
            self.inner.read().unwrap().genop.clone()
        }

        pub fn error(&self) -> Option<StepError> {
            self.inner.read().unwrap().error.clone()
        }

        pub fn push_error(&self, err: StepError) {
            self.inner.write().unwrap().error = Some(err);
        }

        pub fn remove(&self, key: &str) -> Option<Option<Value>> {
            self.inner.write().unwrap().state.remove(key)
        }

        pub fn get_hashed(&self, key: &str) -> Option<Option<Value>> {
            self.inner.read().unwrap().state.get(key).cloned()
        }
    }
}

#[derive(Clone)]
pub struct Step {
    state: state::State,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum StepRunResult<T, E> {
    Data(T),
    Error(E),
}

pub trait UserProvidedError<'a>: StdError + Serialize + Deserialize<'a> + Into<Error> {}

impl<E> UserProvidedError<'_> for E where
    E: for<'a> Deserialize<'a> + StdError + Serialize + Into<Error>
{
}

impl Step {
    pub fn new(state: &HashMap<String, Option<Value>>) -> Self {
        Step {
            state: State::new(state),
        }
    }

    fn new_op(&self, id: &str) -> Op {
        self.state.new_op(id)
    }

    pub(crate) fn error(&self) -> Option<StepError> {
        self.state.error()
    }

    pub(crate) fn genop(&self) -> Vec<GeneratorOpCode> {
        self.state.genop()
    }

    fn remove(&self, key: &str) -> Option<Option<Value>> {
        self.state.remove(key)
    }

    fn push_op(&self, op: GeneratorOpCode) {
        self.state.push_op(op);
    }

    fn push_error(&self, err: StepError) {
        self.state.push_error(err);
    }

    fn get_hashed(&self, key: &str) -> Option<Option<Value>> {
        self.state.get_hashed(key)
    }

    pub async fn run<T, E, F>(&self, id: &str, f: impl FnOnce() -> F) -> Result<T, Error>
    where
        T: for<'a> Deserialize<'a> + Serialize,
        E: for<'a> UserProvidedError<'a>,
        F: Future<Output = Result<T, E>>,
    {
        let op = self.new_op(id);
        let hashed = op.hash();

        if let Some(Some(stored_value)) = self.remove(&hashed) {
            let run_result: StepRunResult<T, E> =
                serde_json::from_value(stored_value).map_err(|e| basic_error!("{}", e))?;

            match run_result {
                StepRunResult::Data(data) => return Ok(data),
                StepRunResult::Error(err) => return Err(err.into()),
            }
        }

        // If we're here, we need to execute the function
        match f().await {
            Ok(result) => {
                let serialized =
                    serde_json::to_value(&result).map_err(|e| basic_error!("{}", e))?;

                self.push_op(GeneratorOpCode {
                    op: Opcode::StepRun,
                    id: hashed,
                    name: id.to_string(),
                    display_name: id.to_string(),
                    data: serialized.into(),
                    opts: json!({}),
                });
                Err(Error::Interrupt(FlowControlError::step_generator()))
            }
            Err(err) => {
                // TODO: need to handle the following errors returned from the user
                // - retry after error
                // - non retriable error

                let serialized_err =
                    serde_json::to_value(&err).map_err(|e| basic_error!("{}", e))?;

                self.push_error(StepError {
                    name: "Step failed".to_string(),
                    message: err.to_string(),
                    stack: None,
                    data: Some(serialized_err),
                });
                Err(Error::Interrupt(FlowControlError::step_generator()))
            }
        }
    }

    #[must_use = "This method returns a Result type, which should be handled and propagated to the caller"]
    pub fn sleep(&self, id: &str, dur: Duration) -> Result<(), Error> {
        let op = self.new_op(id);
        let hashed = op.hash();

        match self.get_hashed(&hashed) {
            // if state already exists, it means we already slept
            Some(_) => Ok(()),

            // TODO: if no state exists, we need to signal to sleep
            None => {
                let opts = json!({
                    "duration": duration::to_string(dur)
                });

                self.push_op(GeneratorOpCode {
                    op: Opcode::Sleep,
                    id: hashed,
                    name: id.to_string(),
                    display_name: id.to_string(),
                    data: None,
                    opts,
                });

                Err(Error::Interrupt(FlowControlError::step_generator()))
            }
        }
    }

    #[must_use = "This method returns a Result type, which should be handled and propagated to the caller"]
    pub fn sleep_until(&self, id: &str, unix_ts_ms: i64) -> Result<(), Error> {
        let op = self.new_op(id);
        let hashed = op.hash();

        match self.get_hashed(&hashed) {
            Some(_) => Ok(()),

            None => {
                let systime = self.unix_ts_to_systime(unix_ts_ms);
                let dur = match systime.duration_since(SystemTime::now()) {
                    Ok(dur) => dur,
                    Err(err) => {
                        return Err(basic_error!("error computing duration for sleep: {}", err));
                    }
                };

                let opts = json!({
                    "duration": duration::to_string(dur)
                });

                self.push_op(GeneratorOpCode {
                    op: Opcode::Sleep,
                    id: hashed,
                    name: id.to_string(),
                    display_name: id.to_string(),
                    data: None,
                    opts,
                });

                Err(Error::Interrupt(FlowControlError::step_generator()))
            }
        }
    }

    #[must_use = "This method returns a Result type, which should be handled and propagated to the caller"]
    pub fn wait_for_event<T: InngestEvent>(
        &self,
        id: &str,
        opts: WaitForEventOpts,
    ) -> Result<Option<Event<T>>, Error> {
        let op = self.new_op(id);
        let hashed = op.hash();

        match self.get_hashed(&hashed) {
            Some(evt) => {
                match evt {
                    None => Ok(None),

                    Some(v) => match serde_json::from_value::<Event<T>>(v.clone()) {
                        Ok(e) => Ok(Some(e)),
                        Err(err) => {
                            // TODO: probably should log this properly
                            println!("error deserializing matched event: {}", err);
                            Ok(None)
                        }
                    },
                }
            }

            None => {
                let mut wait_opts = json!({
                    "event": &opts.event,
                    "timeout": duration::to_string(opts.timeout),
                });
                if let Some(exp) = opts.if_exp {
                    wait_opts["if"] = json!(&exp);
                }

                self.push_op(GeneratorOpCode {
                    op: Opcode::WaitForEvent,
                    id: hashed,
                    name: id.to_string(),
                    display_name: id.to_string(),
                    data: None,
                    opts: wait_opts,
                });

                Err(Error::Interrupt(FlowControlError::step_generator()))
            }
        }
    }

    #[must_use = "This method returns a Result type, which should be handled and propagated to the caller"]
    pub fn invoke<T: for<'de> Deserialize<'de>>(
        &self,
        id: &str,
        opts: InvokeFunctionOpts,
    ) -> Result<T, Error> {
        let op = self.new_op(id);
        let hashed = op.hash();

        match self.get_hashed(&hashed) {
            Some(resp) => match resp {
                None => Err(Error::NoInvokeFunctionResponseError),
                Some(v) => match serde_json::from_value::<T>(v.clone()) {
                    Ok(res) => Ok(res),
                    Err(err) => Err(basic_error!("error deserializing invoke result: {}", err)),
                },
            },

            None => {
                let mut invoke_opts = json!({
                    "function_id": &opts.function_id,
                    "payload": {
                        "data": &opts.data
                    }
                });

                if let Some(timeout) = opts.timeout {
                    invoke_opts["timeout"] = json!(duration::to_string(timeout));
                }

                self.push_op(GeneratorOpCode {
                    op: Opcode::InvokeFunction,
                    id: hashed,
                    name: id.to_string(),
                    display_name: id.to_string(),
                    data: None,
                    opts: invoke_opts,
                });

                Err(Error::Interrupt(FlowControlError::step_generator()))
            }
        }
    }

    fn unix_ts_to_systime(&self, ts: i64) -> SystemTime {
        if ts >= 0 {
            time::UNIX_EPOCH + Duration::from_millis(ts as u64)
        } else {
            // handle negative timestamp
            let nts = Duration::from_millis(-ts as u64);
            time::UNIX_EPOCH - nts
        }
    }

    // TODO: send_event
    // TODO: send_events
}

pub struct WaitForEventOpts {
    pub event: String,
    pub timeout: Duration,
    pub if_exp: Option<String>,
}

pub struct InvokeFunctionOpts {
    pub function_id: String,
    pub data: Value,
    pub timeout: Option<Duration>,
}

struct Op {
    id: String,
    pos: u64,
    // TODO: need an opts as map??
}

impl Op {
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
        let op = Op {
            id: "hello".to_string(),
            pos: 0,
        };

        assert_eq!(op.hash(), "AAF4C61DDCC5E8A2DABEDE0F3B482CD9AEA9434D");
    }

    #[test]
    fn test_op_hash_with_position() {
        let op = Op {
            id: "hello".to_string(),
            pos: 1,
        };

        assert_eq!(op.hash(), "20A9BB9477C4AC565CF084D1614C58BBF0A523FF");
    }
}

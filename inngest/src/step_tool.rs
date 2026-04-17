use std::{
    collections::HashMap,
    error::Error as StdError,
    fmt::{Display, Formatter},
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
    client::Inngest,
    event::{Event, InngestEvent},
    result::{DevError, Error, FlowControlError, StepError},
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
        missing_step: Option<String>,
    }

    impl InnerState {
        fn new_op(&mut self, id: &str) -> Op {
            let pos = self.indices.entry(id.to_string()).or_insert(0);
            let op = Op {
                id: id.to_string(),
                pos: *pos,
            };
            *pos += 1;
            op
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
                    missing_step: None,
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

        pub fn missing_step(&self) -> Option<String> {
            self.inner.read().unwrap().missing_step.clone()
        }

        pub fn push_missing_step(&self, id: String) {
            self.inner.write().unwrap().missing_step = Some(id);
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
    client: Inngest,
    state: state::State,
    target_step_id: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MemoizedStepResult<T> {
    Data { data: T },
    Error { error: StepError },
}

pub trait UserProvidedError<'a>: StdError + Serialize + Deserialize<'a> + Into<Error> {}

impl<E> UserProvidedError<'_> for E where
    E: for<'a> Deserialize<'a> + StdError + Serialize + Into<Error>
{
}

impl Step {
    /// Creates a step helper for the current function execution.
    pub(crate) fn new(
        client: Inngest,
        state: &HashMap<String, Option<Value>>,
        target_step_id: &str,
    ) -> Self {
        Step {
            client,
            state: State::new(state),
            target_step_id: target_step_id.to_string(),
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

    pub(crate) fn missing_step(&self) -> Option<String> {
        self.state.missing_step()
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

    fn push_missing_step(&self, id: String) {
        self.state.push_missing_step(id);
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

        if let Some(stored_value) = self.remove(&hashed) {
            return match parse_memoized_step_result(stored_value, "run step")? {
                MemoizedStepResult::Data { data } => Ok(data),
                MemoizedStepResult::Error { error } => Err(Error::Dev(DevError::Step(error))),
            };
        }

        if self.target_step_id != "step" && self.target_step_id != hashed {
            self.push_missing_step(self.target_step_id.clone());
            return Err(Error::Interrupt(FlowControlError::step_generator()));
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
            Some(resp) => match parse_memoized_step_result(resp, "invoke step")? {
                MemoizedStepResult::Data { data } => Ok(data),
                MemoizedStepResult::Error { error } => Err(Error::Dev(DevError::Step(error))),
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

    /// Sends one event durably and returns the emitted event IDs.
    pub async fn send_event<T: InngestEvent>(
        &self,
        id: &str,
        evt: Event<T>,
    ) -> Result<Vec<String>, Error> {
        self.send_events(id, vec![evt]).await
    }

    /// Sends multiple events durably and returns the emitted event IDs.
    pub async fn send_events<T: InngestEvent>(
        &self,
        id: &str,
        evts: Vec<Event<T>>,
    ) -> Result<Vec<String>, Error> {
        let client = self.client.clone();
        self.run(id, || async move {
            client
                .send_owned_events_with_ids(evts.as_slice())
                .await
                .map_err(StepSendEventError::from)
        })
        .await
    }
}

fn parse_invoke_response<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, Error> {
    match parse_memoized_step_result(Some(value), "invoke step")? {
        MemoizedStepResult::Data { data } => Ok(data),
        MemoizedStepResult::Error { error } => Err(Error::Dev(DevError::Step(error))),
    }
}

fn parse_memoized_step_result<T: for<'de> Deserialize<'de>>(
    value: Option<Value>,
    step_kind: &str,
) -> Result<MemoizedStepResult<T>, Error> {
    let value = value.ok_or_else(|| {
        basic_error!("error parsing memoized {step_kind} result: expected wrapped data or error")
    })?;

    serde_json::from_value::<MemoizedStepResult<T>>(value).map_err(|err| {
        basic_error!("error deserializing memoized {step_kind} result: {}", err)
    })
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

#[derive(Debug, Deserialize, Serialize)]
struct StepSendEventError {
    message: String,
}

impl Display for StepSendEventError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for StepSendEventError {}

impl From<DevError> for StepSendEventError {
    fn from(err: DevError) -> Self {
        let message = match err {
            DevError::Basic(message) => message,
            DevError::Step(err) => err.to_string(),
            DevError::RetryAt(err) => err.to_string(),
            DevError::NoRetry(err) => err.to_string(),
        };

        Self { message }
    }
}

impl From<StepSendEventError> for Error {
    fn from(err: StepSendEventError) -> Self {
        Error::Dev(DevError::Basic(err.message))
    }
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

        base16::encode_lower(res.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Inngest;
    use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
    use serde_json::json;
    use std::{
        net::TcpListener,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };

    #[derive(Clone, Default)]
    struct EventApiState {
        requests: Arc<AtomicUsize>,
        bodies: Arc<Mutex<Vec<Value>>>,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
    struct TestEventData {
        value: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct TestStepFailure {
        message: String,
    }

    impl Display for TestStepFailure {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl StdError for TestStepFailure {}

    impl From<TestStepFailure> for Error {
        fn from(err: TestStepFailure) -> Self {
            Error::Dev(DevError::Basic(err.message))
        }
    }

    #[test]
    fn test_op_hash() {
        let op = Op {
            id: "hello".to_string(),
            pos: 0,
        };

        assert_eq!(op.hash(), "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn test_op_hash_with_position() {
        let op = Op {
            id: "hello".to_string(),
            pos: 1,
        };

        assert_eq!(op.hash(), "20a9bb9477c4ac565cf084d1614c58bbf0a523ff");
    }

    #[test]
    fn repeated_step_ids_start_without_a_suffix() {
        let state = state::State::new(&HashMap::new());

        let first = state.new_op("hello");
        let second = state.new_op("hello");
        let third = state.new_op("hello");

        assert_eq!(first.hash(), "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
        assert_eq!(second.hash(), "20a9bb9477c4ac565cf084d1614c58bbf0a523ff");
        assert_eq!(third.hash(), "7db70735b4beeadfd5cccfe4a5eb48b71acfb404");
    }

    #[tokio::test]
    async fn run_reuses_wrapped_memoized_data() {
        let client = Inngest::new("test-app");
        let state = HashMap::from([(
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d".to_string(),
            Some(json!({ "data": { "value": "hello" } })),
        )]);
        let step = Step::new(client, &state, "step");

        let result: TestEventData = step
            .run("hello", || async {
                Err::<TestEventData, TestStepFailure>(TestStepFailure {
                    message: "step should not rerun".to_string(),
                })
            })
            .await
            .expect("memoized step should return stored data");

        assert_eq!(
            result,
            TestEventData {
                value: "hello".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn run_surfaces_wrapped_memoized_errors_as_step_errors() {
        let client = Inngest::new("test-app");
        let state = HashMap::from([(
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d".to_string(),
            Some(json!({
                "error": {
                    "name": "StepError",
                    "message": "memoized failure",
                    "stack": "trace"
                }
            })),
        )]);
        let step = Step::new(client, &state, "step");

        let result: Result<TestEventData, Error> = step
            .run("hello", || async {
                Err::<TestEventData, TestStepFailure>(TestStepFailure {
                    message: "step should not rerun".to_string(),
                })
            })
            .await;

        match result {
            Err(Error::Dev(DevError::Step(err))) => {
                assert_eq!(err.name, "StepError");
                assert_eq!(err.message, "memoized failure");
                assert_eq!(err.stack.as_deref(), Some("trace"));
            }
            other => panic!("expected memoized step error, got {other:?}"),
        }
    }

    #[test]
    fn wait_for_event_uses_raw_memoized_event_payload() {
        let client = Inngest::new("test-app");
        let state = HashMap::from([(
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d".to_string(),
            Some(json!({
                "name": "test/wait",
                "id": "evt-1",
                "data": { "value": "hello" },
                "ts": 1
            })),
        )]);
        let step = Step::new(client, &state, "step");

        let result = step
            .wait_for_event::<TestEventData>(
                "hello",
                WaitForEventOpts {
                    event: "test/wait".to_string(),
                    timeout: Duration::from_secs(1),
                    if_exp: None,
                },
            )
            .expect("memoized wait should deserialize the raw event");

        let event = result.expect("memoized wait should return an event");
        assert_eq!(event.id.as_deref(), Some("evt-1"));
        assert_eq!(event.data.value, "hello");
    }

    #[test]
    fn wait_for_event_preserves_null_timeout_values() {
        let client = Inngest::new("test-app");
        let state =
            HashMap::from([("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d".to_string(), None)]);
        let step = Step::new(client, &state, "step");

        let result = step
            .wait_for_event::<TestEventData>(
                "hello",
                WaitForEventOpts {
                    event: "test/wait".to_string(),
                    timeout: Duration::from_secs(1),
                    if_exp: None,
                },
            )
            .expect("timed out waits should decode to None");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn send_event_emits_step_run_and_reuses_memoized_result() {
        let server = spawn_event_api().await;
        let client = Inngest::new("test-app")
            .event_api_origin(&server.url)
            .event_key("test-key");
        let step = Step::new(client.clone(), &HashMap::new(), "step");

        let first = step
            .send_event(
                "send-test",
                Event::new(
                    "test/send",
                    TestEventData {
                        value: "hello".to_string(),
                    },
                ),
            )
            .await;

        match first {
            Err(Error::Interrupt(mut flow)) => flow.acknowledge(),
            other => panic!("expected step interruption, got {other:?}"),
        }

        assert_eq!(server.state.requests.load(Ordering::SeqCst), 1);
        assert_eq!(step.genop().len(), 1);
        assert_eq!(step.genop()[0].data, Some(json!(["evt-1"])));

        let stored = HashMap::from([(
            step.genop()[0].id.clone(),
            Some(json!({ "data": ["evt-1"] })),
        )]);
        let memoized_step = Step::new(client, &stored, "step");
        let second = memoized_step
            .send_event(
                "send-test",
                Event::new(
                    "test/send",
                    TestEventData {
                        value: "ignored".to_string(),
                    },
                ),
            )
            .await
            .expect("memoized step should return stored IDs");

        assert_eq!(second, vec!["evt-1".to_string()]);
        assert_eq!(server.state.requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn send_events_posts_an_array_payload() {
        let server = spawn_event_api().await;
        let client = Inngest::new("test-app")
            .event_api_origin(&server.url)
            .event_key("test-key");
        let step = Step::new(client, &HashMap::new(), "step");

        let result = step
            .send_events(
                "send-many",
                vec![
                    Event::new(
                        "test/send.first",
                        TestEventData {
                            value: "first".to_string(),
                        },
                    ),
                    Event::new(
                        "test/send.second",
                        TestEventData {
                            value: "second".to_string(),
                        },
                    ),
                ],
            )
            .await;

        match result {
            Err(Error::Interrupt(mut flow)) => flow.acknowledge(),
            other => panic!("expected step interruption, got {other:?}"),
        }

        assert_eq!(step.genop()[0].data, Some(json!(["evt-1", "evt-2"])));

        let bodies = server.state.bodies.lock().unwrap();
        assert_eq!(bodies.len(), 1);
        assert!(bodies[0].is_array());
        assert_eq!(bodies[0].as_array().unwrap().len(), 2);
    }

    #[test]
    fn invoke_response_returns_data_field() {
        let response = parse_invoke_response::<String>(json!({
            "data": "hello"
        }))
        .expect("invoke response should deserialize data");

        assert_eq!(response, "hello");
    }

    #[test]
    fn invoke_response_returns_error_field() {
        let response = parse_invoke_response::<String>(json!({
            "error": {
                "name": "StepError",
                "message": "invoke failed",
                "stack": "trace"
            }
        }));

        match response {
            Err(Error::Dev(DevError::Step(err))) => {
                assert_eq!(err.name, "StepError");
                assert_eq!(err.message, "invoke failed");
                assert_eq!(err.stack.as_deref(), Some("trace"));
            }
            other => panic!("expected invoke error, got {other:?}"),
        }
    }

    struct TestServer {
        state: EventApiState,
        url: String,
    }

    async fn spawn_event_api() -> TestServer {
        async fn ingest(
            State(state): State<EventApiState>,
            Json(body): Json<Value>,
        ) -> impl IntoResponse {
            state.requests.fetch_add(1, Ordering::SeqCst);
            state.bodies.lock().unwrap().push(body.clone());

            let ids = match body {
                Value::Array(ref events) => (1..=events.len())
                    .map(|idx| format!("evt-{}", idx))
                    .collect::<Vec<_>>(),
                _ => vec!["evt-1".to_string()],
            };

            Json(json!({
                "ids": ids,
                "status": 200
            }))
        }

        let state = EventApiState::default();
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener.local_addr().expect("listener addr should exist");
        let app = Router::new()
            .route("/e/:event_key", post(ingest))
            .with_state(state.clone());

        tokio::spawn(async move {
            axum::Server::from_tcp(listener)
                .expect("server should bind")
                .serve(app.into_make_service())
                .await
                .expect("server should serve");
        });

        TestServer {
            state,
            url: format!("http://{}", addr),
        }
    }
}

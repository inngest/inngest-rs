use crate::{
    client::Inngest,
    event::{Event, InngestEvent},
    step_tool::Step as StepTool,
    utils::duration,
};
use futures::future::BoxFuture;
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use slug::slugify;
use std::{collections::HashMap, fmt::Debug, time::Duration};

// NOTE: should T have Copy trait too?
// so it can do something like `input.event` without moving.
// but the benefit vs effort might be too much for users.
pub struct Input<T: 'static> {
    pub event: Event<T>,
    pub events: Vec<Event<T>>,
    pub ctx: InputCtx,
}

pub struct InputCtx {
    pub env: String,
    pub fn_id: String,
    pub run_id: String,
    pub step_id: String,
    pub attempt: u8,
}

/// A function-config time value accepted by the sync payload.
///
/// Durations are serialized as Inngest time strings such as `5m` or `30s`.
/// Raw strings can be used when the spec accepts an expression or ISO 8601
/// timestamp. Empty string values are rejected when building the sync payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionTime {
    Duration(Duration),
    String(String),
}

impl FunctionTime {
    fn validate(&self, field: &str) -> Result<(), String> {
        if matches!(self, Self::String(value) if value.trim().is_empty()) {
            return Err(format!("{field} cannot be empty"));
        }

        Ok(())
    }
}

impl From<Duration> for FunctionTime {
    fn from(value: Duration) -> Self {
        Self::Duration(value)
    }
}

impl From<String> for FunctionTime {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for FunctionTime {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl Serialize for FunctionTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = match self {
            Self::Duration(duration) => duration::to_string(*duration),
            Self::String(value) => value.clone(),
        };

        serializer.serialize_str(&value)
    }
}

impl<'de> Deserialize<'de> for FunctionTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.is_empty() {
            return Err(D::Error::custom("function time values cannot be empty"));
        }

        Ok(Self::String(value))
    }
}

/// Cancels a function run when a matching event is received.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FunctionCancel {
    pub event: String,
    #[serde(rename = "if", skip_serializing_if = "Option::is_none")]
    pub if_exp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<FunctionTime>,
}

impl FunctionCancel {
    /// Creates a cancellation rule for the given event name.
    pub fn new(event: &str) -> Self {
        Self {
            event: event.to_string(),
            if_exp: None,
            timeout: None,
        }
    }

    /// Adds an expression that must evaluate to `true` to cancel the run.
    pub fn if_exp(mut self, if_exp: &str) -> Self {
        self.if_exp = Some(if_exp.to_string());
        self
    }

    /// Restricts how long the cancellation rule remains active.
    pub fn timeout<T: Into<FunctionTime>>(mut self, timeout: T) -> Self {
        self.timeout = Some(timeout.into());
        self
    }
}

/// Configures event batching for a function trigger.
///
/// Supported sync payloads require `max_size` to be between `1` and `100`.
/// Duration-based timeouts are validated to the spec's `1s` to `60s` range
/// when the sync payload is built.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FunctionBatchEvents {
    #[serde(rename = "maxSize")]
    pub max_size: u32,
    pub timeout: FunctionTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

impl FunctionBatchEvents {
    /// Creates a batch configuration with the required size and timeout.
    pub fn new<T: Into<FunctionTime>>(max_size: u32, timeout: T) -> Self {
        Self {
            max_size,
            timeout: timeout.into(),
            key: None,
        }
    }

    /// Partitions batches by the expression result.
    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }
}

/// Rate-limits how often a function can run within a period.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FunctionRateLimit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub limit: u32,
    pub period: FunctionTime,
}

impl FunctionRateLimit {
    /// Creates a rate-limit configuration with the required limit and period.
    pub fn new<T: Into<FunctionTime>>(limit: u32, period: T) -> Self {
        Self {
            key: None,
            limit,
            period: period.into(),
        }
    }

    /// Partitions rate limits by the expression result.
    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }
}

/// Debounces function execution until a quiet period has elapsed.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FunctionDebounce {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub period: FunctionTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<FunctionTime>,
}

impl FunctionDebounce {
    /// Creates a debounce configuration with the required quiet period.
    pub fn new<T: Into<FunctionTime>>(period: T) -> Self {
        Self {
            key: None,
            period: period.into(),
            timeout: None,
        }
    }

    /// Partitions debounce state by the expression result.
    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }

    /// Caps how long the debounce window can continue to extend.
    pub fn timeout<T: Into<FunctionTime>>(mut self, timeout: T) -> Self {
        self.timeout = Some(timeout.into());
        self
    }
}

/// Controls execution priority relative to other queued runs.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FunctionPriority {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

impl FunctionPriority {
    /// Creates an empty priority configuration.
    pub fn new() -> Self {
        Self { run: None }
    }

    /// Sets the expression used to determine run priority.
    pub fn run(mut self, run: &str) -> Self {
        self.run = Some(run.to_string());
        self
    }
}

impl Default for FunctionPriority {
    fn default() -> Self {
        Self::new()
    }
}

/// Defines how a keyed concurrency group is scoped.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum FunctionConcurrencyScope {
    #[serde(rename = "fn")]
    Function,
    #[serde(rename = "env")]
    Env,
    #[serde(rename = "account")]
    Account,
}

/// Configures one keyed concurrency limit.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FunctionConcurrencyOption {
    pub limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<FunctionConcurrencyScope>,
}

impl FunctionConcurrencyOption {
    /// Creates a keyed concurrency option with the required limit.
    pub fn new(limit: u32) -> Self {
        Self {
            limit,
            key: None,
            scope: None,
        }
    }

    /// Partitions concurrency groups by the expression result.
    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }

    /// Sets the scope for the concurrency group.
    pub fn scope(mut self, scope: FunctionConcurrencyScope) -> Self {
        self.scope = Some(scope);
        self
    }
}

/// Configures how function executions are concurrency-limited.
///
/// Keyed concurrency supports at most two options and is validated when the
/// sync payload is built.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum FunctionConcurrency {
    Limit(u32),
    Keyed(Vec<FunctionConcurrencyOption>),
}

impl FunctionConcurrency {
    /// Creates a concurrency configuration using the numeric shorthand.
    pub fn limit(limit: u32) -> Self {
        Self::Limit(limit)
    }

    /// Creates a concurrency configuration using keyed options.
    pub fn keyed(options: Vec<FunctionConcurrencyOption>) -> Self {
        Self::Keyed(options)
    }
}

/// Throttles function execution to a steady rate.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FunctionThrottle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub limit: u32,
    pub period: FunctionTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burst: Option<u32>,
}

impl FunctionThrottle {
    /// Creates a throttle configuration with the required limit and period.
    pub fn new<T: Into<FunctionTime>>(limit: u32, period: T) -> Self {
        Self {
            key: None,
            limit,
            period: period.into(),
            burst: None,
        }
    }

    /// Partitions throttle groups by the expression result.
    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }

    /// Sets the burst capacity allowed above the steady-state rate.
    pub fn burst(mut self, burst: u32) -> Self {
        self.burst = Some(burst);
        self
    }
}

/// Determines how singleton runs behave when a duplicate key is triggered.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FunctionSingletonMode {
    Skip,
    Cancel,
}

/// Ensures only one run per singleton key is active at a time.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FunctionSingleton {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub mode: FunctionSingletonMode,
}

impl FunctionSingleton {
    /// Creates a singleton configuration with the required behavior mode.
    pub fn new(mode: FunctionSingletonMode) -> Self {
        Self { key: None, mode }
    }

    /// Partitions singleton groups by the expression result.
    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }
}

/// Controls how long a function can wait to start or finish.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct FunctionTimeouts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<FunctionTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish: Option<FunctionTime>,
}

impl FunctionTimeouts {
    /// Creates an empty timeout configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum time from trigger to first execution attempt.
    pub fn start<T: Into<FunctionTime>>(mut self, start: T) -> Self {
        self.start = Some(start.into());
        self
    }

    /// Sets the maximum total runtime allowed for the function.
    pub fn finish<T: Into<FunctionTime>>(mut self, finish: T) -> Self {
        self.finish = Some(finish.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct FunctionOpts {
    pub id: String,
    pub name: Option<String>,
    pub retries: u8,
    pub cancel: Vec<FunctionCancel>,
    pub idempotency: Option<String>,
    pub batch_events: Option<FunctionBatchEvents>,
    pub rate_limit: Option<FunctionRateLimit>,
    pub debounce: Option<FunctionDebounce>,
    pub priority: Option<FunctionPriority>,
    pub concurrency: Option<FunctionConcurrency>,
    pub throttle: Option<FunctionThrottle>,
    pub singleton: Option<FunctionSingleton>,
    pub timeouts: Option<FunctionTimeouts>,
}

impl Default for FunctionOpts {
    fn default() -> Self {
        FunctionOpts {
            id: String::new(),
            name: None,
            retries: 3,
            cancel: Vec::new(),
            idempotency: None,
            batch_events: None,
            rate_limit: None,
            debounce: None,
            priority: None,
            concurrency: None,
            throttle: None,
            singleton: None,
            timeouts: None,
        }
    }
}

impl FunctionOpts {
    /// Creates function options with the provided identifier.
    pub fn new(id: &str) -> Self {
        FunctionOpts {
            id: id.to_string(),
            ..Default::default()
        }
    }

    /// Overrides the display name shown in Inngest.
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Overrides the number of retry attempts the executor should schedule.
    pub fn retries(mut self, retries: u8) -> Self {
        self.retries = retries;
        self
    }

    /// Adds a cancellation rule to the function definition.
    pub fn cancel(mut self, cancel: FunctionCancel) -> Self {
        self.cancel.push(cancel);
        self
    }

    /// Sets an idempotency expression for the function.
    ///
    /// When present, the sync payload omits `rate_limit` to match the spec's
    /// idempotency precedence rules.
    pub fn idempotency(mut self, idempotency: &str) -> Self {
        self.idempotency = Some(idempotency.to_string());
        self
    }

    /// Configures event batching for the function trigger.
    pub fn batch_events(mut self, batch_events: FunctionBatchEvents) -> Self {
        self.batch_events = Some(batch_events);
        self
    }

    /// Configures rate-limiting for the function.
    pub fn rate_limit(mut self, rate_limit: FunctionRateLimit) -> Self {
        self.rate_limit = Some(rate_limit);
        self
    }

    /// Configures debouncing for the function.
    pub fn debounce(mut self, debounce: FunctionDebounce) -> Self {
        self.debounce = Some(debounce);
        self
    }

    /// Configures execution priority for the function.
    pub fn priority(mut self, priority: FunctionPriority) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Configures concurrency limits for the function.
    pub fn concurrency(mut self, concurrency: FunctionConcurrency) -> Self {
        self.concurrency = Some(concurrency);
        self
    }

    /// Configures throttling for the function.
    pub fn throttle(mut self, throttle: FunctionThrottle) -> Self {
        self.throttle = Some(throttle);
        self
    }

    /// Configures singleton execution for the function.
    pub fn singleton(mut self, singleton: FunctionSingleton) -> Self {
        self.singleton = Some(singleton);
        self
    }

    /// Configures execution timeouts for the function.
    pub fn timeouts(mut self, timeouts: FunctionTimeouts) -> Self {
        self.timeouts = Some(timeouts);
        self
    }
}

type Func<T, E> =
    dyn Fn(Input<T>, StepTool) -> BoxFuture<'static, Result<Value, E>> + Send + Sync + 'static;

pub struct ServableFn<T: 'static, E> {
    pub(crate) app_id: String,
    pub(crate) client: Inngest,
    pub opts: FunctionOpts,
    pub trigger: Trigger,
    pub func: Box<Func<T, E>>,
}

impl<T: InngestEvent, E> Debug for ServableFn<T, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServableFn")
            .field("id", &self.opts.id)
            .field("trigger", &self.trigger())
            .finish()
    }
}

impl<T, E> ServableFn<T, E> {
    // TODO: prepend app_id
    pub fn slug(&self) -> String {
        format!("{}-{}", &self.app_id, slugify(self.opts.id.clone()))
    }

    pub fn name(&self) -> String {
        match self.opts.name.clone() {
            Some(name) => name,
            None => self.slug(),
        }
    }

    pub fn trigger(&self) -> Trigger {
        self.trigger.clone()
    }

    pub fn function(&self, serve_origin: &str, serve_path: &str) -> Function {
        let id = format!("{}-{}", &self.app_id, slugify(self.opts.id.clone()));
        let name = match self.opts.name.clone() {
            Some(name) => name,
            None => id.clone(),
        };

        let mut steps = HashMap::new();
        steps.insert(
            "step".to_string(),
            Step {
                id: "step".to_string(),
                name: "step".to_string(),
                runtime: StepRuntime {
                    url: format!("{}{}?fnId={}&stepId=step", serve_origin, serve_path, &id),
                    method: "http".to_string(),
                },
                retries: StepRetry {
                    attempts: self.opts.retries,
                },
            },
        );

        Function {
            id,
            name,
            triggers: vec![self.trigger.clone()],
            steps,
            cancel: self.opts.cancel.clone(),
            idempotency: self.opts.idempotency.clone(),
            batch_events: self.opts.batch_events.clone(),
            rate_limit: if self.opts.idempotency.is_some() {
                None
            } else {
                self.opts.rate_limit.clone()
            },
            debounce: self.opts.debounce.clone(),
            priority: self.opts.priority.clone(),
            concurrency: self.opts.concurrency.clone(),
            throttle: self.opts.throttle.clone(),
            singleton: self.opts.singleton.clone(),
            timeouts: self.opts.timeouts.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Function {
    pub id: String,
    pub name: String,
    pub triggers: Vec<Trigger>,
    pub steps: HashMap<String, Step>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cancel: Vec<FunctionCancel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency: Option<String>,
    #[serde(rename = "batchEvents", skip_serializing_if = "Option::is_none")]
    pub batch_events: Option<FunctionBatchEvents>,
    #[serde(rename = "rateLimit", skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<FunctionRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debounce: Option<FunctionDebounce>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<FunctionPriority>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<FunctionConcurrency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub throttle: Option<FunctionThrottle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub singleton: Option<FunctionSingleton>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeouts: Option<FunctionTimeouts>,
}

impl Function {
    /// Validates supported function config shapes during sync payload
    /// construction.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(batch_events) = &self.batch_events {
            if batch_events.max_size == 0 || batch_events.max_size > 100 {
                return Err(format!(
                    "function {} batchEvents.maxSize must be between 1 and 100",
                    self.id
                ));
            }

            batch_events
                .timeout
                .validate(&format!("function {} batchEvents.timeout", self.id))?;

            if let FunctionTime::Duration(timeout) = &batch_events.timeout {
                if *timeout < Duration::from_secs(1) || *timeout > Duration::from_secs(60) {
                    return Err(format!(
                        "function {} batchEvents.timeout must be between 1s and 60s",
                        self.id
                    ));
                }
            }
        }

        if let Some(FunctionConcurrency::Keyed(options)) = &self.concurrency {
            if options.is_empty() {
                return Err(format!(
                    "function {} concurrency must include at least one keyed option",
                    self.id
                ));
            }

            if options.len() > 2 {
                return Err(format!(
                    "function {} concurrency supports at most two keyed options",
                    self.id
                ));
            }
        }

        for cancel in &self.cancel {
            if let Some(timeout) = &cancel.timeout {
                timeout.validate(&format!("function {} cancel.timeout", self.id))?;
            }
        }

        if let Some(rate_limit) = &self.rate_limit {
            rate_limit
                .period
                .validate(&format!("function {} rateLimit.period", self.id))?;
        }

        if let Some(debounce) = &self.debounce {
            debounce
                .period
                .validate(&format!("function {} debounce.period", self.id))?;

            if let Some(timeout) = &debounce.timeout {
                timeout.validate(&format!("function {} debounce.timeout", self.id))?;
            }
        }

        if let Some(throttle) = &self.throttle {
            throttle
                .period
                .validate(&format!("function {} throttle.period", self.id))?;
        }

        if let Some(timeouts) = &self.timeouts {
            if let Some(start) = &timeouts.start {
                start.validate(&format!("function {} timeouts.start", self.id))?;
            }

            if let Some(finish) = &timeouts.finish {
                finish.validate(&format!("function {} timeouts.finish", self.id))?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Step {
    pub id: String,
    pub name: String,
    pub runtime: StepRuntime,
    pub retries: StepRetry,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StepRuntime {
    pub url: String,
    #[serde(rename = "type")]
    pub method: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StepRetry {
    pub attempts: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Trigger {
    EventTrigger {
        event: String,
        expression: Option<String>,
    },
    CronTrigger {
        cron: String,
    },
}

impl Trigger {
    pub fn event(name: &str) -> Self {
        Trigger::EventTrigger {
            event: name.to_string(),
            expression: None,
        }
    }

    #[allow(unused_variables)]
    pub fn expr(&self, exp: &str) -> Self {
        match self {
            Trigger::EventTrigger { event, expression } => Trigger::EventTrigger {
                event: event.clone(),
                expression: Some(exp.to_string()),
            },
            Trigger::CronTrigger { cron } => Trigger::CronTrigger { cron: cron.clone() },
        }
    }

    pub fn cron(cron: &str) -> Self {
        Trigger::CronTrigger {
            cron: cron.to_string(),
        }
    }
}

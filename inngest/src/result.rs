use std::fmt::{Debug, Display};

use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub(crate) struct SdkResponse {
    pub status: u8,
    pub body: Value,
}

#[derive(Debug)]
pub enum Error {
    Basic(String),
    RetryAt(RetryAfterError),
    NoRetry(NonRetriableError),
}

pub struct StepError {
    pub name: String,
    pub message: String,
    pub stack: Option<String>,
}

pub struct RetryAfterError {
    pub message: String,
    pub retry_after: i64,
    pub cause: Option<String>,
}

impl Display for RetryAfterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Error: {}, retrying after timestamp: {}",
            &self.message, &self.retry_after
        )
    }
}

impl Debug for RetryAfterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cause = match &self.cause {
            None => String::new(),
            Some(c) => c.clone(),
        };

        write!(
            f,
            "Error: {}\nRetrying after timestamp: {}\nCause: {}",
            &self.message, &self.retry_after, &cause
        )
    }
}

pub struct NonRetriableError {
    pub message: String,
    pub cause: Option<String>,
}

impl Display for NonRetriableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {}, not retrying", &self.message)
    }
}

impl Debug for NonRetriableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cause = match &self.cause {
            None => String::new(),
            Some(c) => c.clone(),
        };

        write!(f, "Error: {}\nNo retry\nCause: {}", &self.message, &cause)
    }
}

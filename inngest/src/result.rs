use std::fmt::{Debug, Display};

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize)]
pub struct SdkResponse {
    pub status: u8,
    pub body: Value,
}

impl IntoResponse for SdkResponse {
    fn into_response(self) -> axum::response::Response {
        match self.status {
            200 => (StatusCode::OK, Json(self.body)).into_response(),
            206 => (StatusCode::PARTIAL_CONTENT, Json(self.body)).into_response(),
            _ => (StatusCode::BAD_REQUEST, Json(json!("Unknown response"))).into_response(),
        }
    }
}

#[derive(Debug)]
pub enum InggestError {
    Basic(String),
    RetryAt(RetryAfterError),
    NoRetry(NonRetryableError),

    // These are not expected to be used by users
    #[allow(private_interfaces)]
    Interrupt(FlowControlError),
}

#[derive(Debug)]
pub(crate) enum FlowControlError {
    StepGenerator,
}

impl IntoResponse for InggestError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!("NOT IMPLEMENTED")),
        )
            .into_response()
    }
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

pub struct NonRetryableError {
    pub message: String,
    pub cause: Option<String>,
}

impl Display for NonRetryableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {}, not retrying", &self.message)
    }
}

impl Debug for NonRetryableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cause = match &self.cause {
            None => String::new(),
            Some(c) => c.clone(),
        };

        write!(f, "Error: {}\nNo retry\nCause: {}", &self.message, &cause)
    }
}

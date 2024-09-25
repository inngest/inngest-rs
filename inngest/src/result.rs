use std::fmt::{Debug, Display};

use axum::{
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};

use crate::step_tool::GeneratorOpCode;

#[derive(Serialize)]
pub struct SdkResponse {
    pub status: u16,
    pub body: Value,
}

impl IntoResponse for SdkResponse {
    fn into_response(self) -> axum::response::Response {
        let mut headers = HeaderMap::new();
        let sdk = format!("rust:{}", env!("CARGO_PKG_VERSION"));
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert("x-inngest-framework", HeaderValue::from_static("axum"));
        headers.insert("x-inngest-sdk", HeaderValue::from_str(&sdk).unwrap());

        match self.status {
            200 => (StatusCode::OK, headers, Json(self.body)).into_response(),
            206 => (StatusCode::PARTIAL_CONTENT, headers, Json(self.body)).into_response(),
            400 => (StatusCode::BAD_REQUEST, headers, Json(self.body)).into_response(),
            500 => (StatusCode::INTERNAL_SERVER_ERROR, headers, Json(self.body)).into_response(),
            _ => (StatusCode::BAD_REQUEST, Json(json!("Unknown response"))).into_response(),
        }
    }
}

#[derive(Debug)]
pub enum InngestError {
    Basic(String),
    RetryAt(RetryAfterError),
    NoRetry(NonRetryableError),

    // These are not expected to be used by users
    #[allow(private_interfaces)]
    Interrupt(FlowControlError),
}

#[derive(Debug)]
pub(crate) enum FlowControlError {
    StepGenerator(Vec<GeneratorOpCode>),
    StepError(StepError),
}

impl FlowControlError {
    pub fn new_step_generator(opcodes: impl Into<Vec<GeneratorOpCode>>) -> Self {
        FlowControlError::StepGenerator(opcodes.into())
    }
}

impl IntoResponse for InngestError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!("NOT IMPLEMENTED")),
        )
            .into_response()
    }
}

#[derive(Serialize, Debug)]
pub struct StepError {
    pub name: String,
    pub message: String,
    pub stack: Option<String>,
    // not part of the spec but it's used in the Go SDK to deserialize into the original user error
    pub data: Option<serde_json::Value>,
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

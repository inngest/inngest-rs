use std::fmt::{Debug, Display};

use axum::{
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};

use crate::header;

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
        headers.insert(header::INNGEST_FRAMEWORK, HeaderValue::from_static("axum"));
        headers.insert(header::INNGEST_SDK, HeaderValue::from_str(&sdk).unwrap());
        headers.insert(header::INNGEST_REQ_VERSION, HeaderValue::from_static("1"));

        match self.status {
            200 => (StatusCode::OK, headers, Json(self.body)).into_response(),
            206 => (StatusCode::PARTIAL_CONTENT, headers, Json(self.body)).into_response(),
            400 => (StatusCode::BAD_REQUEST, headers, Json(self.body)).into_response(),
            500 => (StatusCode::INTERNAL_SERVER_ERROR, headers, Json(self.body)).into_response(),
            _ => (StatusCode::BAD_REQUEST, Json(json!("Unknown response"))).into_response(),
        }
    }
}

/// Error type that the user (developer) is supposed to interact with
#[derive(Debug)]
pub enum DevError {
    /// A catch-all error type for business logic errors
    Basic(String),
    /// Error that controls how the function will be retried
    RetryAt(RetryAfterError),
    /// Error that does not allow the function to be retried
    NoRetry(NonRetryableError),
}

/// Result type that the user (developer) is supposed to interact with
pub type DevResult<T> = std::result::Result<T, DevError>;

/// Result type that includes internal/control flow errors
pub type InngestResult<T> = std::result::Result<T, Error>;

#[must_use]
#[derive(Debug)]
pub enum Error {
    /// Developer facing errors
    Dev(DevError),
    /// Internal only. Used for invoked functions that don't have a response
    NoInvokeFunctionResponseError,
    /// Internal only. These are not expected to be used by users. These must be propagated to their callers
    Interrupt(FlowControlError),
}

impl From<DevError> for Error {
    fn from(err: DevError) -> Self {
        Error::Dev(err)
    }
}

/// Create a basic error using format! syntax
#[macro_export]
macro_rules! basic_error {
    ($($arg:tt)*) => {
        $crate::result::Error::Dev($crate::result::DevError::Basic(
            format!($($arg)*),
        ))
    };
}

/// Correctly propagate the flow control error while providing the user with a simple error
#[macro_export]
macro_rules! into_dev_result {
    ($err:expr) => {
        match $err {
            Ok(val) => Ok(val),
            Err(e) => match e {
                $crate::result::Error::Interrupt(_)
                | $crate::result::Error::NoInvokeFunctionResponseError => return Err(e),
                $crate::result::Error::Dev(s) => Err(s),
            },
        }
    };
}

#[derive(Debug)]
pub enum FlowControlVariant {
    StepGenerator,
}

#[derive(Debug)]
pub struct FlowControlError {
    acknowledged: bool,
    pub variant: FlowControlVariant,
}

impl FlowControlError {
    /// create a new flow control error for a step generator
    pub(crate) fn step_generator() -> Self {
        FlowControlError {
            acknowledged: false,
            variant: FlowControlVariant::StepGenerator,
        }
    }

    ///  This must be called before the error is dropped
    pub(crate) fn acknowledge(&mut self) {
        self.acknowledged = true;
    }
}

impl Drop for FlowControlError {
    fn drop(&mut self) {
        if !self.acknowledged {
            if std::thread::panicking() {
                // we don't want to panic in a panic, because calling panic within a destructor during
                // a panic will cause the program to abort
                // TODO: also add error! level tracing here
                println!("Flow control error was not acknowledged");
            } else {
                panic!("Flow control error was not acknowledged.
                This is a developer error.
                You should ensure that you return the flow control error within Inngest funcitons to the caller as soon as they're received.");
            }
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::Dev(err) => match err {
                DevError::Basic(msg) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(msg))),
                DevError::RetryAt(_err) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!("retry after error")),
                ),
                DevError::NoRetry(_err) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!("no retry error")),
                ),
            },
            Error::NoInvokeFunctionResponseError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!("No invoke response")),
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!("NOT IMPLEMENTED")),
            ),
        }
        .into_response()
    }
}

#[derive(Serialize, Debug, Clone)]
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

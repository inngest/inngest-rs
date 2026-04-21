use std::{
    error::Error as StdError,
    fmt::{Debug, Display},
    time::Duration,
};

use axum::{
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    header,
    version::{self, EXECUTION_VERSION},
};

#[derive(Serialize)]
pub struct SdkResponse {
    pub status: u16,
    pub body: Value,
}

/// Error type that the user (developer) is supposed to interact with
#[derive(Debug)]
pub enum DevError {
    /// A catch-all error type for business logic errors
    Basic(String),
    /// A memoized step error surfaced back through replay.
    Step(StepError),
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
    ParallelSkip,
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

    /// Create a branch-local flow control error used to skip unrelated work
    /// while searching a parallel group for a targeted step.
    pub(crate) fn parallel_skip() -> Self {
        FlowControlError {
            acknowledged: false,
            variant: FlowControlVariant::ParallelSkip,
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
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        // TODO: framework might need to change
        headers.insert(header::INNGEST_FRAMEWORK, HeaderValue::from_static("axum"));
        headers.insert(
            header::INNGEST_SDK,
            HeaderValue::from_str(&version::sdk()).unwrap(),
        );
        headers.insert(
            header::INNGEST_REQ_VERSION,
            HeaderValue::from_static(EXECUTION_VERSION),
        );

        match self {
            Error::Dev(err) => match err {
                DevError::Basic(msg) => call_error_response(
                    headers,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    false,
                    None,
                    StepError::new("Error", msg),
                ),
                DevError::Step(err) => {
                    call_error_response(headers, StatusCode::BAD_REQUEST, true, None, err)
                }
                DevError::RetryAt(retry) => {
                    let retry_after = HeaderValue::from(retry.after.as_secs());

                    call_error_response(
                        headers,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        false,
                        Some(retry_after),
                        StepError {
                            name: "RetryAfterError".to_string(),
                            message: retry.message,
                            stack: retry.cause,
                            data: None,
                        },
                    )
                }
                DevError::NoRetry(err) => call_error_response(
                    headers,
                    StatusCode::BAD_REQUEST,
                    true,
                    None,
                    StepError {
                        name: "NonRetryableError".to_string(),
                        message: err.message,
                        stack: err.cause,
                        data: None,
                    },
                ),
            },
            Error::NoInvokeFunctionResponseError => call_error_response(
                headers,
                StatusCode::INTERNAL_SERVER_ERROR,
                false,
                None,
                StepError::new("NoInvokeFunctionResponseError", "No invoke response"),
            ),
            Error::Interrupt(flow) => call_error_response(
                headers,
                StatusCode::INTERNAL_SERVER_ERROR,
                false,
                None,
                StepError::new(
                    "FlowControlError",
                    format!("Unhandled flow control error: {:?}", flow.variant),
                ),
            ),
            // No other variants exist today, but keep all call errors on the spec shape.
        }
        .into_response()
    }
}

fn call_error_response(
    mut headers: HeaderMap,
    status: StatusCode,
    no_retry: bool,
    retry_after: Option<HeaderValue>,
    body: StepError,
) -> (StatusCode, HeaderMap, Json<Value>) {
    headers.insert(
        header::INNGEST_NO_RETRY,
        HeaderValue::from_static(if no_retry { "true" } else { "false" }),
    );

    if let Some(retry_after) = retry_after {
        headers.insert(header::RETRY_AFTER, retry_after);
    }

    (status, headers, Json(json!(body)))
}

/// A serializable error payload returned to Inngest on failed calls.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct StepError {
    pub name: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    // not part of the spec but it's used in the Go SDK to deserialize into the original user error
    #[serde(skip_serializing)]
    pub data: Option<serde_json::Value>,
}

impl StepError {
    pub(crate) fn new(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            message: message.into(),
            stack: None,
            data: None,
        }
    }
}

impl Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.message)
    }
}

impl StdError for StepError {}

/// A retriable developer error that asks Inngest to delay the next attempt.
pub struct RetryAfterError {
    pub message: String,
    pub after: Duration,
    pub cause: Option<String>,
}

impl Display for RetryAfterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Error: {}, retrying after {}s",
            &self.message,
            self.after.as_secs()
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
            "Error: {}\nRetrying after {}s:\nCause: {}",
            &self.message,
            self.after.as_secs(),
            &cause
        )
    }
}

/// A non-retriable developer error that should fail the run immediately.
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use hyper::body::to_bytes;

    async fn response_json(response: axum::response::Response) -> Value {
        let body = to_bytes(response.into_body())
            .await
            .expect("response body should be readable");
        serde_json::from_slice(&body).expect("response body should be valid json")
    }

    #[tokio::test]
    async fn basic_errors_are_retriable_and_use_spec_shape() {
        let response = Error::Dev(DevError::Basic("boom".to_string())).into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.headers().get(header::INNGEST_SDK).unwrap(),
            HeaderValue::from_str(&version::sdk()).unwrap()
        );
        assert_eq!(
            response.headers().get(header::INNGEST_REQ_VERSION).unwrap(),
            HeaderValue::from_static(EXECUTION_VERSION)
        );
        assert_eq!(
            response.headers().get(header::INNGEST_NO_RETRY).unwrap(),
            HeaderValue::from_static("false")
        );

        assert_eq!(
            response_json(response).await,
            json!({
                "name": "Error",
                "message": "boom"
            })
        );
    }

    #[tokio::test]
    async fn non_retryable_errors_return_bad_request() {
        let response = Error::Dev(DevError::NoRetry(NonRetryableError {
            message: "stop".to_string(),
            cause: Some("because".to_string()),
        }))
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers().get(header::INNGEST_NO_RETRY).unwrap(),
            HeaderValue::from_static("true")
        );
        assert!(response.headers().get(header::RETRY_AFTER).is_none());

        assert_eq!(
            response_json(response).await,
            json!({
                "name": "NonRetryableError",
                "message": "stop",
                "stack": "because"
            })
        );
    }

    #[tokio::test]
    async fn memoized_step_errors_return_bad_request_and_preserve_shape() {
        let response = Error::Dev(DevError::Step(StepError {
            name: "StepError".to_string(),
            message: "step failed".to_string(),
            stack: Some("trace".to_string()),
            data: Some(json!({ "ignored": true })),
        }))
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers().get(header::INNGEST_NO_RETRY).unwrap(),
            HeaderValue::from_static("true")
        );
        assert!(response.headers().get(header::RETRY_AFTER).is_none());

        assert_eq!(
            response_json(response).await,
            json!({
                "name": "StepError",
                "message": "step failed",
                "stack": "trace"
            })
        );
    }

    #[tokio::test]
    async fn retry_after_errors_preserve_retry_after_header() {
        let response = Error::Dev(DevError::RetryAt(RetryAfterError {
            message: "try later".to_string(),
            after: Duration::from_secs(42),
            cause: Some("slow dependency".to_string()),
        }))
        .into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.headers().get(header::INNGEST_NO_RETRY).unwrap(),
            HeaderValue::from_static("false")
        );
        assert_eq!(
            response.headers().get(header::RETRY_AFTER).unwrap(),
            HeaderValue::from_static("42")
        );

        assert_eq!(
            response_json(response).await,
            json!({
                "name": "RetryAfterError",
                "message": "try later",
                "stack": "slow dependency"
            })
        );
    }
}

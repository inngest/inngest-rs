use std::fmt::{Debug, Display};

use axum::{
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};

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
pub enum SimpleError {
    Basic(String),
    RetryAt(RetryAfterError),
    NoRetry(NonRetryableError),

    // Used for invoked functions that don't have a response
    NoInvokeFunctionResponseError,
}

#[derive(Debug)]
pub enum InngestError {
    Simple(SimpleError),

    /// These are not expected to be used by users. These must be propagated to their callers
    Interrupt(FlowControlError),
}

impl From<SimpleError> for InngestError {
    fn from(err: SimpleError) -> Self {
        InngestError::Simple(err)
    }
}

#[macro_export]
macro_rules! basic_error {
    ($($arg:tt)*) => {
        $crate::result::InngestError::Simple($crate::result::SimpleError::Basic(
            format!($($arg)*),
        ))
    };
}

// Correctly propagate the flow control error while providing the user with a simple error
#[macro_export]
macro_rules! simplify_err {
    ($err:expr) => {
        match $err {
            Ok(val) => Ok(val),
            Err(e) => match e {
                $crate::result::InngestError::Interrupt(_) => return Err(e),
                $crate::result::InngestError::Simple(s) => Err(s),
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
    pub(crate) fn step_generator() -> Self {
        FlowControlError {
            acknowledged: false,
            variant: FlowControlVariant::StepGenerator,
        }
    }

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

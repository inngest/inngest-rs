use crate::{
    basic_error,
    handler::{Handler, IntrospectResult, RunQueryParams, SyncQueryParams, SyncResponse},
    header::{self, Headers},
    result::{Error, SdkResponse}, version,
};

use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::{fmt::Debug, sync::Arc};

const FRAMEWORK: &str = "axum";

pub async fn introspect<T, E>(
    hmap: HeaderMap,
    State(handler): State<Arc<Handler<T, E>>>,
    raw: String,
) -> Result<IntrospectResult, Error> {
    let headers = Headers::from(hmap);
    handler.introspect(&headers, FRAMEWORK, &raw).await
}

pub async fn register<T, E>(
    hmap: HeaderMap,
    Query(query): Query<SyncQueryParams>,
    State(handler): State<Arc<Handler<T, E>>>,
) -> Result<SyncResponse, String> {
    // convert the http headers into a generic hashmap
    let headers = Headers::from(hmap);
    handler.sync(&headers, &query, FRAMEWORK).await
}

pub async fn invoke<T, E>(
    hmap: HeaderMap,
    Query(query): Query<RunQueryParams>,
    State(handler): State<Arc<Handler<T, E>>>,
    raw: String,
) -> Result<SdkResponse, Error>
where
    T: for<'de> Deserialize<'de> + Debug,
    E: Into<Error>,
{
    let headers = Headers::from(hmap);
    match serde_json::from_str(&raw) {
        Ok(body) => handler.run(&headers, &query, &raw, &body).await,
        Err(_err) => Err(basic_error!("failed to parse body as JSON")),
    }
}

// Response conversion
impl IntoResponse for SdkResponse {
    fn into_response(self) -> axum::response::Response {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(header::INNGEST_FRAMEWORK, HeaderValue::from_static(&FRAMEWORK));
        headers.insert(header::INNGEST_SDK, HeaderValue::from_str(&version::sdk()).unwrap());
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

impl IntoResponse for SyncResponse {
    fn into_response(self) -> axum::response::Response {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(header::INNGEST_FRAMEWORK, HeaderValue::from_static(&FRAMEWORK));
        headers.insert(header::INNGEST_SDK, HeaderValue::from_str(&version::sdk()).unwrap());
        headers.insert(header::INNGEST_REQ_VERSION, HeaderValue::from_static("1"));

        (StatusCode::OK, headers, Json(&self)).into_response()
    }
}

impl IntoResponse for IntrospectResult {
    fn into_response(self) -> axum::response::Response {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(header::INNGEST_FRAMEWORK, HeaderValue::from_static(&FRAMEWORK));
        headers.insert(header::INNGEST_SDK, HeaderValue::from_str(&version::sdk()).unwrap());
        headers.insert(header::INNGEST_REQ_VERSION, HeaderValue::from_static("1"));

        (StatusCode::OK, headers, Json(&self)).into_response()
    }
}

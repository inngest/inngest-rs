use crate::{
    basic_error,
    handler::{Handler, IntrospectResult, RunQueryParams, SyncQueryParams, SyncResponse},
    header::Headers,
    result::{Error, SdkResponse},
};

use axum::{
    extract::{Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
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

use crate::{
    handler::{Handler, RunQueryParams},
    header::Headers,
    result::{Error, SdkResponse},
};

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use std::{fmt::Debug, sync::Arc};

// TODO:
// provide a macro for simple import into Axum routes

pub async fn register<T, E>(
    hmap: HeaderMap,
    State(handler): State<Arc<Handler<T, E>>>,
) -> Result<(), String> {
    // convert the http headers into a generic hashmap
    let headers = Headers::from(hmap);
    handler.sync(&headers, "axum").await
}

pub async fn invoke<T, E>(
    hmap: HeaderMap,
    Query(query): Query<RunQueryParams>,
    State(handler): State<Arc<Handler<T, E>>>,
    Json(body): Json<Value>,
) -> Result<SdkResponse, Error>
where
    T: for<'de> Deserialize<'de> + Debug,
    E: Into<Error>,
{
    let headers = Headers::from(hmap);
    handler.run(&headers, query, &body).await
}

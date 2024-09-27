use crate::{
    basic_error,
    handler::{Handler, RunQueryParams},
    result::{Error, SdkResponse, DevError},
};

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, fmt::Debug, sync::Arc};

// TODO:
// provide a macro for simple import into Axum routes

pub async fn register<T, E>(
    hmap: HeaderMap,
    State(handler): State<Arc<Handler<T, E>>>,
) -> Result<(), String> {
    // convert the http headers into a generic hashmap
    let mut headers: HashMap<String, String> = HashMap::new();
    for head in hmap.iter() {
        let key = head.0.to_string().to_lowercase();
        if let Ok(v) = head.1.to_str() {
            headers.insert(key, v.to_string());
        }
    }

    handler.sync(&headers, "axum").await
}

pub async fn invoke<T, E>(
    Query(query): Query<RunQueryParams>,
    State(handler): State<Arc<Handler<T, E>>>,
    Json(body): Json<Value>,
) -> Result<SdkResponse, Error>
where
    T: for<'de> Deserialize<'de> + Debug,
    E: Into<Error>,
{
    handler.run(query, &body)
}

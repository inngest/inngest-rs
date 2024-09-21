use crate::{
    event::InngestEvent,
    handler::{Handler, RunQueryParams},
    result::{Error, SdkResponse},
};

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

// TODO:
// provide a macro for simple import into Axum routes

pub async fn register<T: InngestEvent>(
    hmap: HeaderMap,
    State(handler): State<Arc<Handler<T>>>,
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

pub async fn invoke<T: InngestEvent>(
    Query(query): Query<RunQueryParams>,
    State(handler): State<Arc<Handler<T>>>,
    Json(body): Json<Value>,
) -> Result<SdkResponse, Error> {
    handler
        .run(query, &body)
        .map_err(|err| Error::Basic(format!("{:?}", err)))
}

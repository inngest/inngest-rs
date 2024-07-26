use crate::{
    event::InngestEvent,
    handler::{Handler, RunQueryParams}, result::Error,
};

use axum::{
    extract::{Query, State},
    Json,
};
use serde_json::Value;
use std::sync::Arc;

// TODO:
// provide a macro for simple import into Axum routes

pub async fn register<T: InngestEvent>(
    State(handler): State<Arc<Handler<T>>>,
) -> Result<(), String> {
    handler.sync("axum").await
}

pub async fn invoke<T: InngestEvent>(
    Query(query): Query<RunQueryParams>,
    State(handler): State<Arc<Handler<T>>>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, Error> {
    // TODO: update result types?
    handler
        .run(query, &body)
        .map(|res| Json(res))
        .map_err(|err| Error::Basic(format!("{:?}", err)))
}

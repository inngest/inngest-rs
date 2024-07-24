use axum::{
    extract::{Query, State},
    Json,
};
use serde_json::Value;

use crate::{
    event::InngestEvent, router::Handler
};

use super::RunQueryParams;
use std::sync::Arc;

pub async fn register<T: InngestEvent>(
    State(handler): State<Arc<Handler<T>>>,
) -> Result<(), String> {
    handler.sync("axum").await
}

pub async fn invoke<T: InngestEvent>(
    Query(query): Query<RunQueryParams>,
    State(handler): State<Arc<Handler<T>>>,
    Json(body): Json<Value>,
) -> Result<(), String> { // TODO: update result types?
    handler.run(query, &body)
        .map(|_| ())
        .map_err(|err| format!("{:?}", err))
}

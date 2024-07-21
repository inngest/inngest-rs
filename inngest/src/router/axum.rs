use axum::{
    extract::{Query, State},
    Json,
};
use serde_json::Value;

use crate::{
    event::InngestEvent,
    function::{Input, InputCtx},
    router::Handler,
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
) -> Result<(), String> {
    println!("Body: {:#?}", body);

    match handler.funcs.iter().find(|f| f.slug() == query.fn_id) {
        None => Err(format!("no function registered as ID: {}", query.fn_id)),
        Some(func) => {
            println!("Slug: {}", func.slug());
            println!("Trigger: {:?}", func.trigger());
            println!("Event: {:?}", func.event(&body["event"]));

            match func.event(&body["event"]) {
                None => Err("failed to parse event".to_string()),
                Some(evt) => (func.func)(Input {
                    event: evt,
                    events: vec![],
                    ctx: InputCtx {
                        fn_id: String::new(),
                        run_id: String::new(),
                        step_id: String::new(),
                    },
                })
                .map(|_res| ()),
            }
        }
    }
}

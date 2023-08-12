pub mod axum;

use crate::function::ServableFunction;

pub struct Handler {
    app_name: String,
    funcs: Vec<Box<dyn ServableFunction>>,
}

use crate::function::Function;

use serde::{Deserialize, Serialize};
use std::default::Default;

#[derive(Debug, Deserialize, Serialize)]
pub struct Request {
    #[serde(rename = "appName")]
    pub app_name: String,
    #[serde(rename = "deployType")]
    pub deploy_type: String,
    pub url: String,
    pub v: String,
    pub sdk: String,
    pub framework: String,
    pub functions: Vec<Function>,
}

impl Default for Request {
    fn default() -> Self {
        let version = env!("CARGO_PKG_VERSION");

        Request {
            app_name: "InngestApp".to_string(),
            deploy_type: "ping".to_string(),
            url: "".to_string(),
            v: "1".to_string(),
            sdk: format!("rust:v{}", version),
            framework: "rust".to_string(),
            functions: vec![],
        }
    }
}

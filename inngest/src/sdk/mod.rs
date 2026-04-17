use crate::function::Function;
use crate::version;

use serde::{Deserialize, Serialize};
use std::default::Default;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Request {
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
        Request {
            app_name: "RustDev".to_string(),
            deploy_type: "ping".to_string(),
            url: String::new(),
            v: version::SYNC_VERSION.to_string(),
            sdk: version::sdk(),
            framework: "rust".to_string(),
            functions: vec![],
        }
    }
}

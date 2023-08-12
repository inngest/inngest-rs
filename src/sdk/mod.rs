use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Request {
    #[serde(rename = "appName")]
    app_name: String,
    #[serde(rename = "deployType")]
    deploy_type: String,
    url: String,
    v: String,
    sdk: String,
    framework: String,
}

#[derive(Clone)]
pub struct Inngest {
    pub app_id: String,
    pub api_base_url: Option<String>,
    pub event_api_base_url: Option<String>,
    pub event_key: Option<String>,
    pub env: Option<String>,
    pub is_dev: Option<bool>,
}

impl Inngest {
    pub fn new(app_id: &str) -> Self {
        Inngest {
            app_id: app_id.to_string(),
            api_base_url: None,
            event_api_base_url: None,
            event_key: None,
            env: None,
            is_dev: None,
        }
    }
}

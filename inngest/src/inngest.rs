pub struct Inngest {
    pub app_id: String,
    pub base_url: Option<String>,
    pub event_key: Option<String>,
    pub env: Option<String>,
    pub is_dev: Option<bool>,
}

impl Inngest {
    pub fn new(app_id: String) -> Self {
        Inngest {
            app_id,
            base_url: None,
            event_key: None,
            env: None,
            is_dev: None,
        }
    }
}

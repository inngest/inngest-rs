pub(crate) struct Config {
    // client side
    api_origin: Option<String>,
    event_api_origin: Option<String>,
    event_key: Option<String>,
    is_dev: Option<bool>,
    // server side
    signing_key: Option<String>,
    serve_origin: Option<String>,
    serve_path: Option<String>,
}

impl Config {
    fn read_env_str(key: &str) -> Option<String> {
        None
    }
}

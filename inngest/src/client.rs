use crate::{
    config::Config,
    event::{Event, InngestEvent},
    result::DevError,
};

const EVENT_API_ORIGIN: &str = "https://inn.gs";

#[derive(Clone)]
pub struct Inngest {
    pub app_id: String,
    api_origin: Option<String>,
    event_api_origin: Option<String>,
    event_key: Option<String>,
    env: Option<String>,
    dev: Option<String>,
    http: reqwest::Client,
}

impl Inngest {
    pub fn new(app_id: &str) -> Self {
        // initialize variable values here using environment variables
        let api_origin = Config::api_origin();
        let event_api_origin = Config::event_api_origin();
        let event_key = Config::event_key();
        let env = Config::env();
        // TODO: allow updating dev server url here
        // https://www.inngest.com/docs/sdk/environment-variables#inngest-dev
        let dev = Config::dev().map(|v| v);

        Inngest {
            app_id: app_id.to_string(),
            api_origin,
            event_api_origin,
            event_key,
            env,
            dev,
            http: reqwest::Client::new(),
        }
    }

    pub fn api_origin(mut self, url: &str) -> Self {
        self.api_origin = Some(url.to_string());
        self
    }

    pub fn event_api_origin(mut self, url: &str) -> Self {
        self.event_api_origin = Some(url.to_string());
        self
    }

    pub fn event_key(mut self, key: &str) -> Self {
        self.event_key = Some(key.to_string());
        self
    }

    pub fn env(mut self, e: &str) -> Self {
        self.env = Some(e.to_string());
        self
    }

    pub fn dev(mut self, dev: &str) -> Self {
        self.dev = Some(dev.to_string());
        self
    }

    // TODO: make the result return something properly
    pub async fn send_event<T: InngestEvent>(&self, evt: &Event<T>) -> Result<(), DevError> {
        self.http
            .post(format!(
                "{}/e/{}",
                self.evt_api_origin(),
                self.evt_api_key()
            ))
            .json(&evt)
            .send()
            .await
            .map(|_| ())
            .map_err(|err| DevError::Basic(format!("{}", err)))
    }

    // TODO: make the result return something properly
    pub async fn send_events<T: InngestEvent>(&self, evts: &[&Event<T>]) -> Result<(), DevError> {
        self.http
            .post(format!(
                "{}/e/{}",
                self.evt_api_origin(),
                self.evt_api_key()
            ))
            .json(&evts)
            .send()
            .await
            .map(|_| ())
            .map_err(|err| DevError::Basic(format!("{}", err)))
    }

    fn evt_api_origin(&self) -> String {
        if let Some(dev) = self.dev.clone() {
            return dev;
        }

        if let Some(endpoint) = self.event_api_origin.clone() {
            return endpoint;
        }

        EVENT_API_ORIGIN.to_string()
    }

    fn evt_api_key(&self) -> String {
        if let Some(_) = self.dev.clone() {
            return "test".to_string();
        }

        match self.event_key.clone() {
            Some(key) => key,
            None => "test".to_string(),
        }
    }
}

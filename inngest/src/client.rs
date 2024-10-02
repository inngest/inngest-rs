use url::Url;

use crate::{
    config::Config,
    event::{Event, InngestEvent},
    handler::Kind,
    result::DevError,
};

const API_ORIGIN_DEV: &str = "http://127.0.0.1:8288";
const EVENT_API_ORIGIN: &str = "https://inn.gs";
const API_ORIGIN: &str = "https://api.inngest.com";

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
        // if the value is a URL, use it. otherwise set a default URL
        let dev = Config::dev().map(|v| match Url::parse(&v) {
            Ok(val) => val.to_string(),
            Err(_) => API_ORIGIN_DEV.to_string(),
        });

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
        let url = match Url::parse(dev) {
            Ok(val) => Some(val.to_string()),
            Err(_) => Some(API_ORIGIN_DEV.to_string()),
        };
        self.dev = url;
        self
    }

    // TODO: make the result return something properly
    pub async fn send_event<T: InngestEvent>(&self, evt: &Event<T>) -> Result<(), DevError> {
        self.http
            .post(format!(
                "{}/e/{}",
                self.inngest_evt_api_origin(),
                self.inngest_evt_api_key()
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
                self.inngest_evt_api_origin(),
                self.inngest_evt_api_key()
            ))
            .json(&evts)
            .send()
            .await
            .map(|_| ())
            .map_err(|err| DevError::Basic(format!("{}", err)))
    }

    pub(crate) fn inngest_api_origin(&self, kind: Kind) -> String {
        if let Some(dev) = self.dev.clone() {
            return dev;
        }

        if let Some(endpoint) = self.api_origin.clone() {
            return endpoint;
        }

        match kind {
            Kind::Dev => API_ORIGIN_DEV.to_string(),
            Kind::Cloud => API_ORIGIN.to_string(),
        }
    }

    fn inngest_evt_api_origin(&self) -> String {
        if let Some(dev) = self.dev.clone() {
            return dev;
        }

        if let Some(endpoint) = self.event_api_origin.clone() {
            return endpoint;
        }

        EVENT_API_ORIGIN.to_string()
    }

    fn inngest_evt_api_key(&self) -> String {
        if let Some(_) = self.dev.clone() {
            return "test".to_string();
        }

        match self.event_key.clone() {
            Some(key) => key,
            None => "test".to_string(),
        }
    }
}

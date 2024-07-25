pub mod event;
pub mod function;
pub mod handler;
pub mod result;
pub mod sdk;
pub mod serve;

use event::{Event, InngestEvent};
use result::Error;

#[derive(Clone)]
pub struct Inngest {
    app_id: String,
    api_base_url: Option<String>,
    event_api_base_url: Option<String>,
    event_key: Option<String>,
    env: Option<String>,
    is_dev: Option<bool>,
    http: reqwest::Client,
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
            http: reqwest::Client::new(),
        }
    }

    pub fn api_base_url(mut self, url: &str) -> Self {
        self.api_base_url = Some(url.to_string());
        self
    }

    pub fn event_api_base_url(mut self, url: &str) -> Self {
        self.event_api_base_url = Some(url.to_string());
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

    pub fn is_dev(mut self, dev: bool) -> Self {
        self.is_dev = Some(dev);
        self
    }

    // TODO: make the result return something properly
    pub async fn send_event<T: InngestEvent>(&self, evt: &Event<T>) -> Result<(), Error> {
        self.http
            .post("http://127.0.0.1:8288/e/test")
            .json(&evt)
            .send()
            .await
            .map(|_| ())
            .map_err(|err| Error::Basic(err.to_string()))
    }

    // TODO: make the result return something properly
    pub async fn send_events<T: InngestEvent>(&self, evts: &[&Event<T>]) -> Result<(), Error> {
        self.http
            .post("http://127.0.0.1:8288/e/test")
            .json(&evts)
            .send()
            .await
            .map(|_| ())
            .map_err(|err| Error::Basic(err.to_string()))
    }
}

pub mod event;
pub mod function;
pub mod handler;
pub mod result;
pub mod sdk;
pub mod serve;

use std::env;

use event::{Event, InngestEvent};
use result::Error;

#[derive(Clone)]
pub struct Inngest {
    app_id: String,
    api_origin: Option<String>,
    event_api_origin: Option<String>,
    event_key: Option<String>,
    env: Option<String>,
    is_dev: Option<bool>,
    http: reqwest::Client,
}

impl Inngest {
    pub fn new(app_id: &str) -> Self {
        // initialize variable values here using environment variables
        let api_origin = Self::read_env("INNGEST_API_ORIGIN");
        let event_api_origin = Self::read_env("INNGEST_EVENT_API_ORIGIN");
        let event_key = Self::read_env("INNGEST_EVENT_KEY");
        let env = Self::read_env("INNGEST_ENV");
        // TODO: allow updating dev server url here
        // https://www.inngest.com/docs/sdk/environment-variables#inngest-dev
        let is_dev = match env::var("INNGEST_DEV") {
            Ok(_) => Some(true),
            Err(_) => None,
        };

        Inngest {
            app_id: app_id.to_string(),
            api_origin,
            event_api_origin,
            event_key,
            env,
            is_dev,
            http: reqwest::Client::new(),
        }
    }

    fn read_env(key: &str) -> Option<String> {
        match env::var(key) {
            Ok(str) => Some(str),
            Err(err) => {
                println!("Error reading environment variable {}: {}", key, err);
                None
            }
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

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

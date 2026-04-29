use serde_json::Value;
use slug::slugify;
use std::future::Future;
use url::Url;

use crate::{
    config::Config,
    event::{Event, InngestEvent},
    function::{FunctionOpts, Input, ServableFn, Trigger},
    handler::Kind,
    result::DevError,
    step_tool::Step as StepTool,
};

const API_ORIGIN_DEV: &str = "http://127.0.0.1:8288";
pub(crate) const EVENT_API_ORIGIN: &str = "https://inn.gs";
pub(crate) const API_ORIGIN: &str = "https://api.inngest.com";

/// The response returned by the Inngest event ingestion API.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct SendEventResponse {
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(default)]
    pub status: u16,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct Inngest {
    id: String,
    pub(crate) api_origin: Option<String>,
    pub(crate) event_api_origin: Option<String>,
    pub(crate) event_key: Option<String>,
    pub(crate) env: Option<String>,
    pub(crate) dev: Option<String>,
    http: reqwest::Client,
}

impl Inngest {
    pub fn new(id: &str) -> Self {
        // initialize variable values here using environment variables
        let api_origin = Config::api_origin();
        let event_api_origin = Config::event_api_origin();
        let event_key = Config::event_key();
        let env = Config::env();
        let dev = Config::dev().and_then(|v| Self::normalize_dev_value(&v));

        Inngest {
            id: id.to_string(),
            api_origin,
            event_api_origin,
            event_key,
            env,
            dev,
            http: reqwest::Client::new(),
        }
    }

    pub fn app_id(&self) -> String {
        slugify(self.id.clone())
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
        self.dev = Self::normalize_dev_value(dev).or_else(|| Some(API_ORIGIN_DEV.to_string()));
        self
    }

    pub(crate) fn mode(&self) -> Kind {
        if self.dev.is_some() {
            Kind::Dev
        } else {
            Kind::Cloud
        }
    }

    pub fn create_function<
        T: 'static,
        E,
        F: Future<Output = Result<Value, E>> + Send + Sync + 'static,
    >(
        &self,
        opts: FunctionOpts,
        trigger: Trigger,
        func: impl Fn(Input<T>, StepTool) -> F + Send + Sync + 'static,
    ) -> ServableFn<T, E> {
        use futures::future::FutureExt;

        let app_id = self.app_id();
        ServableFn {
            app_id,
            client: self.clone(),
            opts,
            trigger,
            func: Box::new(move |input, step| func(input, step).boxed()),
            on_failure: None,
        }
    }

    /// Sends a single event to the configured Inngest event API.
    pub async fn send_event<T: InngestEvent>(
        &self,
        evt: &Event<T>,
    ) -> Result<SendEventResponse, DevError> {
        self.send_payload(evt).await
    }

    /// Sends multiple events to the configured Inngest event API.
    pub async fn send_events<T: InngestEvent>(
        &self,
        evts: &[&Event<T>],
    ) -> Result<SendEventResponse, DevError> {
        self.send_payload(evts).await
    }

    pub(crate) async fn send_owned_events_with_ids<T: InngestEvent>(
        &self,
        evts: &[Event<T>],
    ) -> Result<Vec<String>, DevError> {
        self.send_payload(evts).await.map(|response| response.ids)
    }

    async fn send_payload<T: serde::Serialize + ?Sized>(
        &self,
        payload: &T,
    ) -> Result<SendEventResponse, DevError> {
        let event_url = self.event_api_url();
        let response = self
            .http
            .post(event_url)
            .json(payload)
            .send()
            .await
            .map_err(|err| DevError::Basic(format!("{}", err)))?;

        let http_status = response.status();
        let body: SendEventResponse = response.json().await.map_err(|err| {
            DevError::Basic(format!("error decoding event send response: {}", err))
        })?;

        if !http_status.is_success() || body.status != 200 || body.error.is_some() {
            let message = body.error.unwrap_or_else(|| {
                format!(
                    "unexpected event send response (http {}, body {})",
                    http_status.as_u16(),
                    body.status
                )
            });
            return Err(DevError::Basic(message));
        }

        Ok(body)
    }

    fn event_api_url(&self) -> String {
        let origin = self.inngest_evt_api_origin();
        let event_key = self.inngest_evt_api_key();

        match Url::parse(&origin).and_then(|url| url.join(&format!("e/{}", event_key))) {
            Ok(url) => url.to_string(),
            Err(_) => format!("{}/e/{}", origin.trim_end_matches('/'), event_key),
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
        if self.dev.is_some() {
            return "test".to_string();
        }

        match self.event_key.clone() {
            Some(key) => key,
            None => "test".to_string(),
        }
    }

    pub(crate) fn inngest_api_origin(&self) -> String {
        if let Some(dev) = self.dev.clone() {
            return dev;
        }

        if let Some(endpoint) = self.api_origin.clone() {
            return endpoint;
        }

        match self.mode() {
            Kind::Dev => API_ORIGIN_DEV.to_string(),
            Kind::Cloud => API_ORIGIN.to_string(),
        }
    }

    fn normalize_dev_value(value: &str) -> Option<String> {
        let value = value.trim();
        if value.is_empty() || value == "0" {
            return None;
        }

        match Url::parse(value) {
            Ok(val) => Some(val.to_string()),
            Err(_) => Some(API_ORIGIN_DEV.to_string()),
        }
    }
}

#[cfg(test)]
mod dev_mode_tests {
    use super::*;

    #[test]
    fn normalize_dev_value_treats_zero_as_cloud_mode() {
        assert_eq!(Inngest::normalize_dev_value("0"), None);
        assert_eq!(Inngest::normalize_dev_value(""), None);
    }

    #[test]
    fn normalize_dev_value_uses_default_dev_origin_for_non_zero_flags() {
        assert_eq!(
            Inngest::normalize_dev_value("1"),
            Some(API_ORIGIN_DEV.to_string())
        );
    }

    #[test]
    fn normalize_dev_value_preserves_explicit_dev_origin() {
        assert_eq!(
            Inngest::normalize_dev_value("http://example.com"),
            Some("http://example.com/".to_string())
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::post, Json, Router};
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};
    use std::net::TcpListener;

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct TestEventData {
        value: String,
    }

    #[tokio::test]
    async fn send_event_returns_send_event_response() {
        async fn ingest(Json(_body): Json<Value>) -> Json<Value> {
            Json(json!({
                "ids": ["evt-1"],
                "status": 200
            }))
        }

        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener.local_addr().expect("listener addr should exist");
        let app = Router::new().route("/e/:event_key", post(ingest));

        tokio::spawn(async move {
            axum::Server::from_tcp(listener)
                .expect("server should bind")
                .serve(app.into_make_service())
                .await
                .expect("server should serve");
        });

        let client = Inngest::new("test-app")
            .event_api_origin(&format!("http://{}", addr))
            .event_key("test-key");
        let response = client
            .send_event(&Event::new(
                "test/send",
                TestEventData {
                    value: "hello".to_string(),
                },
            ))
            .await
            .expect("send_event should return the parsed response");

        assert_eq!(
            response,
            SendEventResponse {
                ids: vec!["evt-1".to_string()],
                status: 200,
                error: None,
            }
        );
    }

    #[test]
    fn event_api_url_normalizes_trailing_slash() {
        let with_slash = Inngest::new("test-app")
            .event_api_origin("http://127.0.0.1:8288/")
            .event_key("test-key");
        let without_slash = Inngest::new("test-app")
            .event_api_origin("http://127.0.0.1:8288")
            .event_key("test-key");

        assert_eq!(
            with_slash.event_api_url(),
            "http://127.0.0.1:8288/e/test-key"
        );
        assert_eq!(
            without_slash.event_api_url(),
            "http://127.0.0.1:8288/e/test-key"
        );
    }
}

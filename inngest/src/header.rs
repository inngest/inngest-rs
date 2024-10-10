use std::collections::HashMap;

use crate::handler::Kind;

pub(crate) const CONTENT_TYPE: &str = "content-type";
pub(crate) const RETRY_AFTER: &str = "retry-after";
// pub(crate) const SERVER_TIMING: &str = "server-timing";
// pub(crate) const USER_AGENT: &str = "user-agent";
pub(crate) const HOST: &str = "host";

// Inngest specific ones
// pub(crate) const INNGEST_ENV: &str = "x-inngest-env";
// pub(crate) const INNGEST_EXPECTED_SERVER_KIND: &str = "x-inngest-expected-server-kind";
pub(crate) const INNGEST_FRAMEWORK: &str = "x-inngest-framework";
// pub(crate) const INNGEST_NO_RETRY: &str = "x-inngest-no-retry";
pub(crate) const INNGEST_REQ_VERSION: &str = "x-inngest-req-version";
pub(crate) const INNGEST_SDK: &str = "x-inngest-sdk";
pub(crate) const INNGEST_SERVER_KIND: &str = "x-inngest-server-kind";
pub(crate) const INNGEST_SIGNATURE: &str = "x-inngest-signature";
// pub(crate) const INNGEST_SYNC_KIND: &str = "x-inngest-sync-kind";

#[derive(Debug)]
pub struct Headers(HashMap<String, String>);

impl Headers {
    pub fn server_kind(&self) -> Kind {
        match self.0.get(INNGEST_SERVER_KIND) {
            Some(val) => match val.as_str() {
                "cloud" => Kind::Cloud,
                _ => Kind::Dev,
            },
            None => Kind::Dev,
        }
    }

    pub fn signature(&self) -> Option<String> {
        self.0.get(INNGEST_SIGNATURE).map(|v| v.clone())
    }

    pub fn host(&self) -> Option<String> {
        self.0.get(HOST).map(|v| v.clone())
    }
}

impl From<axum::http::HeaderMap> for Headers {
    fn from(hmap: axum::http::HeaderMap) -> Self {
        let mut headers = HashMap::new();
        for head in hmap.iter() {
            let key = head.0.to_string().to_lowercase();
            if let Ok(v) = head.1.to_str() {
                headers.insert(key, v.to_string());
            }
        }

        Headers(headers)
    }
}

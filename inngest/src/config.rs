use std::env;

// client side
const INNGEST_API_ORIGIN: &str = "INNGEST_API_ORIGIN";
const INNGEST_EVENT_API_ORIGIN: &str = "INNGEST_EVENT_API_ORIGIN";
const INNGEST_EVENT_KEY: &str = "INNGEST_EVENT_KEY";
const INNGEST_ENV: &str = "INNGEST_ENV";
const INNGEST_DEV: &str = "INNGEST_DEV";

// server side
const INNGEST_SIGNING_KEY: &str = "INNGEST_SIGNING_KEY";
const INNGEST_SERVE_ORIGIN: &str = "INNGEST_SERVE_ORIGIN";
const INNGEST_SERVE_PATH: &str = "INNGEST_SERVE_PATH";

// TODO: default values

pub(crate) struct Config {}

impl Config {
    pub fn api_origin() -> Option<String> {
        Self::read_env_str(INNGEST_API_ORIGIN)
    }

    pub fn event_api_origin() -> Option<String> {
        Self::read_env_str(INNGEST_EVENT_API_ORIGIN)
    }

    pub fn event_key() -> Option<String> {
        Self::read_env_str(INNGEST_EVENT_KEY)
    }

    pub fn env() -> Option<String> {
        Self::read_env_str(INNGEST_ENV)
    }

    pub fn is_dev() -> Option<bool> {
        Self::read_env_bool(INNGEST_DEV)
    }

    pub fn signing_key() -> Option<String> {
        Self::read_env_str(INNGEST_SIGNING_KEY)
    }

    pub fn serve_origin() -> Option<String> {
        Self::read_env_str(INNGEST_SERVE_ORIGIN)
    }

    pub fn serve_path() -> Option<String> {
        Self::read_env_str(INNGEST_SERVE_PATH)
    }

    // helper methods
    fn read_env_str(key: &str) -> Option<String> {
        match env::var(key) {
            Ok(str) => Some(str),
            Err(err) => {
                if let env::VarError::NotUnicode(v) = err {
                    println!("Invalid environment variable value '{}': {:?}", key, v);
                }

                None
            }
        }
    }

    fn read_env_bool(key: &str) -> Option<bool> {
        if let Some(v) = Self::read_env_str(key) {
            return match v.parse::<bool>() {
                Ok(b) => Some(b),
                Err(err) => {
                    println!("Error parsing variable '{}' as bool: {:?}", key, err);
                    None
                }
            };
        }

        None
    }
}

use std::{
    collections::HashMap,
    time::{self, Duration, SystemTime},
};

use hmac::{Hmac, Mac};
use regex::Regex;
use sha1::Digest;
use sha2::Sha256;

use crate::{basic_error, result::Error};

type HmacSha256 = Hmac<Sha256>;

pub struct Signature {
    sig: String,
    key: String,
    body: String,
    re: Regex,
}

impl Signature {
    pub fn new(key: &str) -> Self {
        let re = Regex::new(r"^signkey-.+-").unwrap();

        Signature {
            sig: String::new(),
            key: key.to_string(),
            body: String::new(),
            re,
        }
    }

    pub fn sig(mut self, sig: &str) -> Self {
        self.sig = sig.to_string();
        self
    }

    pub fn body(mut self, body: &str) -> Self {
        self.body = body.to_string();
        self
    }

    pub fn hash(&self) -> Result<String, Error> {
        match self.re.find(&self.key) {
            Some(mat) => {
                let prefix = mat.as_str();
                let key = self.normalize_key(&self.key);

                match base16::decode(key.as_bytes()) {
                    Ok(de) => {
                        let mut hasher = Sha256::new();
                        hasher.update(de);

                        let sum = hasher.finalize();
                        let enc = base16::encode_lower(&sum);

                        Ok(format!("{}{}", prefix, enc))
                    }
                    // TODO: handle errors better
                    Err(_err) => Err(basic_error!("decode error")),
                }
            }
            // TODO: handle errors better
            None => Err(basic_error!("hash failure")),
        }
    }

    // TODO: implement signature validation
    pub fn verify(&self, ignore_ts: bool) -> Result<(), Error> {
        let smap = self.sig_map();

        match smap.get("t") {
            Some(tsstr) => match tsstr.parse::<i64>() {
                Ok(ts) => {
                    let now = SystemTime::now();
                    let from = self.unix_ts_to_systime(ts);

                    match now.duration_since(from) {
                        Ok(dur) => {
                            println!(
                                "DEBUG:\n  Timestamp: {}\n  Now: {:#?}\n  From: {:#?}\n  Duration: {:#?}\n  SMap: {:#?}\n  Ignore: {}\n  Within Range: {}",
                                ts, now, from, dur, smap, ignore_ts, dur <= Duration::from_secs(60 * 5)
                            );
                            if ignore_ts || dur <= Duration::from_secs(60 * 5) {
                                match self.sign(ts, &self.key, &self.body) {
                                    Ok(sig) => {
                                        if sig == self.sig {
                                            Ok(())
                                        } else {
                                            // TODO: handle errors better
                                            Err(basic_error!("signature doesn't match"))
                                        }
                                    }
                                    // TODO: handle errors better
                                    Err(_err) => Err(basic_error!("error signing body")),
                                }
                            } else {
                                // TODO: handle errors better
                                Err(basic_error!("invalid sig"))
                            }
                        }
                        // TODO: handle errors better
                        Err(_err) => Err(basic_error!("error parsing time elasped for sig")),
                    }
                }
                // TODO: handle errors better
                Err(_err) => Err(basic_error!("error parsing ts as integer")),
            },
            // TODO: handle errors better
            None => Err(basic_error!("no ts field for signature")),
        }
    }

    fn normalize_key(&self, key: &str) -> String {
        self.re.replace(key, "").to_string()
    }

    fn sig_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();

        for attrs in self.sig.split("&") {
            let mut parts = attrs.split("=");

            if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                map.insert(key.to_string(), val.to_string());
            }
        }

        map
    }

    fn unix_ts_to_systime(&self, ts: i64) -> SystemTime {
        if ts >= 0 {
            time::UNIX_EPOCH + Duration::from_millis(ts as u64)
        } else {
            // handle negative timestamp
            let nts = Duration::from_millis(-ts as u64);
            time::UNIX_EPOCH - nts
        }
    }

    fn sign(&self, unix_ts: i64, signing_key: &str, body: &str) -> Result<String, Error> {
        let key = self.normalize_key(signing_key);
        let mut mac =
            HmacSha256::new_from_slice(&key.as_bytes()).expect("HMAC can take key of any size");
        mac.update(format!("{}{}", body, unix_ts).as_bytes());

        let sum = mac.finalize();
        let sig = base16::encode_lower(&sum.into_bytes());

        Ok(format!("t={}&s={}", unix_ts, sig))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    const SIGNING_KEY: &str =
        "signkey-test-8ee2262a15e8d3c42d6a840db7af3de2aab08ef632b32a37a687f24b34dba3ff";
    const HASHED_SIGNING_KEY: &str =
        "signkey-test-e4bf4a2e7f55c7eb954b6e72f8f69628fbc409fe7da6d0f6958770987dcf0e02";
    const SIGNATURE: &str =
        "t=1689920619&s=31df77f5b1b029de4bfce3a77e0517aa4ce0f5e2195a6467fc126a489ca2330b";

    #[test]
    fn test_hashed_signing_key() {
        let sig = Signature::new(SIGNING_KEY);
        let hashed = sig.hash();
        assert!(hashed.is_ok());
        assert_eq!(HASHED_SIGNING_KEY, hashed.unwrap());
    }

    fn event() -> Value {
        json!({
            "id": "",
            "name": "inngest/scheduled.timer",
            "data": {},
            "user": {},
            "ts": 1_674_082_830_001 as i64,
            "v": "1"
        })
    }

    fn body() -> String {
        let evt = event();

        let body = json!({
            "ctx": {
                "fn_id": "local-testing-local-cron",
                "run_id": "01GQ3HTEZ01M7R8Z9PR1DMHDN1",
                "step_id": "step"
            },
            "event": &evt,
            "events": [&evt],
            "steps": {},
            "use_api": false
        });

        serde_json::to_string(&body).expect("JSON should serialize to string")
    }

    #[test]
    fn test_verify_if_signature_is_valid() {
        let body = body();
        let sig = Signature::new(SIGNING_KEY).sig(SIGNATURE).body(&body);
        let res = sig.verify(true);
        assert!(res.is_ok());
    }

    #[test]
    fn test_verify_if_signature_is_expired() {
        let body = body();
        let sig = Signature::new(SIGNING_KEY).sig(SIGNATURE).body(&body);
        let res = sig.verify(false);
        assert!(res.is_err());
    }

    #[test]
    fn test_verify_if_signature_is_invalid() {
        let body = body();
        let invalid_sig = format!("{}hello", SIGNATURE);
        let sig = Signature::new(SIGNING_KEY).sig(&invalid_sig).body(&body);
        let res = sig.verify(true);
        assert!(res.is_err());
    }

    #[test]
    fn test_verify_for_random_input() {
        let body = body();
        let sig = Signature::new(SIGNING_KEY).sig("10").body(&body);
        let res = sig.verify(true);
        assert!(res.is_err());
    }
}

use regex::Regex;
use serde_json::Value;
use sha1::Digest;
use sha2::Sha256;

use crate::{basic_error, result::Error};

pub struct Signature {
    sig: String,
    key: String,
    body: Value,
    re: Regex,
}

impl Signature {
    pub fn new(sig: &str, key: &str, body: &Value) -> Self {
        let re = Regex::new(r"^signkey-.+-").unwrap();

        Signature {
            sig: sig.to_string(),
            key: key.to_string(),
            body: body.clone(),
            re,
        }
    }

    fn hash(&self) -> Result<String, Error> {
        match self.re.find(&self.key) {
            Some(mat) => {
                let prefix = mat.as_str();
                let key = self.normalize_key();

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
        Ok(())
    }

    fn normalize_key(&self) -> String {
        self.re.replace(&self.key, "").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SIGNING_KEY: &str =
        "signkey-test-8ee2262a15e8d3c42d6a840db7af3de2aab08ef632b32a37a687f24b34dba3ff";
    const HASHED_SIGNING_KEY: &str =
        "signkey-test-e4bf4a2e7f55c7eb954b6e72f8f69628fbc409fe7da6d0f6958770987dcf0e02";
    const SIGNATURE: &str =
        "t=1689920619&s=31df77f5b1b029de4bfce3a77e0517aa4ce0f5e2195a6467fc126a489ca2330b";

    #[test]
    fn test_hashed_signing_key() {
        let body = body();
        let sig = Signature::new(SIGNATURE, SIGNING_KEY, &body);
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

    fn body() -> Value {
        let evt = event();

        json!({
            "ctx": {
                "fn_id": "local-testing-local-cron",
                "run_id": "01GQ3HTEZ01M7R8Z9PR1DMHDN1",
                "step_id": "step"
            },
            "event": &evt,
            "events": [&evt],
            "steps": {},
            "use_api": false
        })
    }

    #[test]
    fn test_verify_if_signature_is_valid() {
        let body = body();
        let sig = Signature::new(SIGNATURE, SIGNING_KEY, &body);
        let res = sig.verify(true);
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn test_verify_if_signature_is_expired() {
        let body = body();
        let sig = Signature::new(SIGNATURE, SIGNING_KEY, &body);
        let res = sig.verify(false);
        assert!(res.is_err());
    }

    #[ignore]
    #[test]
    fn test_verify_if_signature_is_invalid() {
        let body = body();
        let invalid_sig = format!("{}hello", SIGNATURE);
        let sig = Signature::new(&invalid_sig, SIGNING_KEY, &body);
        let res = sig.verify(true);
        assert!(res.is_err());
    }

    #[ignore]
    #[test]
    fn test_verify_for_random_input() {
        let body = body();
        let sig = Signature::new("10", SIGNING_KEY, &body);
        let res = sig.verify(true);
        assert!(res.is_err());
    }
}

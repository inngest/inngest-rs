use serde_json::Value;

use crate::result::Error;

pub struct Signature {
    sig: String,
    key: String,
    body: Value,
}

impl Signature {
    pub fn new(sig: &str, key: &str, body: &Value) -> Self {
        Signature {
            sig: sig.to_string(),
            key: key.to_string(),
            body: body.clone(),
        }
    }

    // TODO: implement signature validation
    pub fn verify(&self) -> Result<(), Error> {
        Ok(())
    }
}

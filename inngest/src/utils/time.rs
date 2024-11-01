use std::time::SystemTime;

#[allow(dead_code)]
pub(crate) fn now_ms() -> i64 {
    let start = SystemTime::now();
    match start.duration_since(std::time::UNIX_EPOCH) {
        Ok(dur) => dur.as_millis() as i64,
        Err(_) => 0,
    }
}

pub(crate) fn now() -> i64 {
    let start = SystemTime::now();
    match start.duration_since(std::time::UNIX_EPOCH) {
        Ok(dur) => dur.as_secs() as i64,
        Err(_) => 0,
    }
}

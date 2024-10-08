use std::time::SystemTime;

pub(crate) fn now() -> i64 {
    let start = SystemTime::now();
    match start.duration_since(std::time::UNIX_EPOCH) {
        Ok(dur) => dur.as_millis() as i64,
        Err(_) => 0,
    }
}

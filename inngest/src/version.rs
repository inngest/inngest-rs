pub(crate) const EXECUTION_VERSION: &str = "2";
pub(crate) const SYNC_VERSION: &str = "0.1";

pub(crate) fn sdk() -> String {
    format!("rust:v{}", env!("CARGO_PKG_VERSION"))
}

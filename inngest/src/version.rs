pub(crate) fn sdk() -> String {
    format!("rust:{}", env!("CARGO_PKG_VERSION"))
}

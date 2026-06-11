/// Initializes logging for the example browsers: warn level by default,
/// overridable through `RUST_LOG`.
pub fn init() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .env()
        .init()
        .unwrap_or_default();
}

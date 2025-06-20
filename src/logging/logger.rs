use tracing_subscriber::Layer;
use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry,
};
use once_cell::sync::OnceCell;
use crate::logging::config::LogConfig;

/// one_cell const for init cheking of logger
static LOGGER_INITIALIZED: OnceCell<()> = OnceCell::new();

/// setup looger function
pub fn setup_logger() -> anyhow::Result<()> {
    if LOGGER_INITIALIZED.get().is_some() {
        return Ok(());
    }

    // default logger config
    let config = LogConfig::default();

    let console_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .boxed();


    // Layers filter
    let filter_layer = EnvFilter::try_new(&config.level)?;

    // Initialize
    let subscriber = Registry::default()
        .with(filter_layer)
        .with(console_layer);

    subscriber.init();

    // Set Logger const
    LOGGER_INITIALIZED.set(()).expect("Init error of logger");
    
    Ok(())
}
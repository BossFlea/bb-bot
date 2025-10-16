use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    Layer as _, filter, fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _,
};

pub fn init_log() -> WorkerGuard {
    let file_appender = tracing_appender::rolling::daily("logs/", "bot.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let crate_filter = filter::Targets::new().with_targets([("bb_bot", Level::DEBUG)]);

    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_filter(crate_filter.clone());
    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_filter(crate_filter);

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();

    guard
}

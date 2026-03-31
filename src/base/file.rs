use std::str::FromStr;

use tracing::{error, info, Level};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::{self, time::OffsetTime},
    prelude::*,
};

use time::macros::{format_description, offset};

pub struct Logger {
    _guard: WorkerGuard,
}

pub async fn init(log_level: &str, log_path: &str, file_name:&str) -> Logger {
    let rust_log = format!("{},h2=off,tower=off,hyper=off,tokio-cron-scheduler=off", log_level);
    unsafe { std::env::set_var("RUST_LOG", rust_log) };

    // std::env::set_var("RUST_LOG", "h2=off,tower=off,hyper=off");

    // 启用对log crate的适配
    // tracing_log::LogTracer::init().expect("Failed to set logger");

    // let file_appender = tracing_appender::rolling::daily(c, file_name);

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .filename_prefix(file_name)
        .filename_suffix("log")
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .build(log_path)
        .unwrap();

    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let time_fmt = OffsetTime::new(
        offset!(+8),
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"),
    );

    let subscriber = fmt::Subscriber::builder()
        .with_max_level(Level::from_str(log_level).unwrap_or(Level::DEBUG))
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .with_timer(time_fmt.clone())
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        // .with_thread_ids(true)
        // .with_thread_names(true)
        .finish()
        // add additional writers
        .with(
            fmt::Layer::default()
                .json()
                .with_timer(time_fmt.clone())
                // .with_thread_ids(true)
                // .with_thread_names(true)
                .flatten_event(true)
                .with_file(true)
                .with_line_number(true)
                .with_target(false)
                .with_ansi(true)
                .with_span_list(true)
                .with_writer(file_writer),
        );

    tracing::subscriber::set_global_default(subscriber)
        .expect("Unable to set global tracing subscriber");

    std::panic::set_hook(Box::new(|panic| {
        if let Some(location) = panic.location() {
            error!(
                message = %panic,
                panic.file = location.file(),
                panic.line = location.line(),
                panic.column = location.column(),
            );
        } else {
            error!(message = %panic);
        }
    }));

    info!("tracing init success.");

    Logger { _guard: guard }
}

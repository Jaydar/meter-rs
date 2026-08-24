use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use tracing::error;
use tracing_subscriber::{
    fmt::{self, time::OffsetTime},
    prelude::*,
    EnvFilter,
};

use time::macros::{format_description, offset};

struct FileWriter {
    file: Arc<Mutex<Option<File>>>,
    path: PathBuf,
}

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file = self.file.lock().map_err(|err| io::Error::other(anyhow::Error::msg(err.to_string()).context("lock log file failed")))?;
        if file.is_none() {
            *file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?,
            );
        }
        if let Some(file) = file.as_mut() {
            file.write_all(buf)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut file = self.file.lock().map_err(|err| io::Error::other(anyhow::Error::msg(err.to_string()).context("lock log file failed")))?;
        if let Some(file) = file.as_mut() {
            file.flush()?;
        }
        Ok(())
    }
}

pub fn init() {
    static _init: OnceLock<()> = OnceLock::new();

    _init.get_or_init(|| {
        let level = std::env::var("RUST_LOG").unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                "trace".to_string()
            } else {
                "error".to_string()
            }
        });

        let file = Arc::new(Mutex::new(None));
        let log_path = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|path| path.join("meter-rs.log")))
            .unwrap_or_else(|| PathBuf::from("meter-rs.log"));

        let time_fmt = OffsetTime::new(
            offset!(+8),
            format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"),
        );

        let subscriber = tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .with_timer(time_fmt.clone())
                    .with_target(false)
                    .with_ansi(true)
                    .with_writer(std::io::stdout)
                    .with_filter(EnvFilter::new(level.clone())),
            )
            .with(
                fmt::layer()
                    .with_timer(time_fmt)
                    .with_target(false)
                    .with_ansi(false)
                    .with_writer(move || FileWriter { file: file.clone(), path: log_path.clone() })
                    .with_filter(EnvFilter::new("trace")),
            );

        if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
            eprintln!("set global tracing subscriber failed: {}", err);
            panic!("set global tracing subscriber failed: {}", err);
        }

        std::panic::set_hook(Box::new(|panic| {
            let location = panic.location().map(|location| format!(" at {}:{}", location.file(), location.line())).unwrap_or_default();
            if let Some(message) = panic.payload().downcast_ref::<&str>() {
                error!("panic{}: {}", location, message);
                return;
            }
            if let Some(message) = panic.payload().downcast_ref::<String>() {
                error!("panic{}: {}", location, message);
                return;
            }
            error!("panic{}: unknown payload", location);
        }));
    });
}

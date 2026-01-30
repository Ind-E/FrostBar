use crate::BAR_NAMESPACE;
use notify_rust::Notification;
use std::path::{Path, PathBuf};
use tracing::warn;
use tracing_subscriber::{EnvFilter, Layer};
use tracing_subscriber::{
    fmt::{self},
    registry::LookupSpan,
    reload,
};

type BoxedLayer<S> =
    Box<dyn tracing_subscriber::layer::Layer<S> + Send + Sync + 'static>;
pub type LogHandle<S> = reload::Handle<Option<BoxedLayer<S>>, S>;

const MAX_LOG_FILES: usize = 10;

pub fn init_tracing<S>(config_dir: &Path, handle: &LogHandle<S>) -> PathBuf
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    let log_dir = config_dir.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let mut log_indices: Vec<u64> = std::fs::read_dir(&log_dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|entry| {
                    entry.file_name().to_str()?.split('-').last()?.parse().ok()
                })
                .collect()
        })
        .unwrap_or_default();
    log_indices.sort_unstable();

    if log_indices.len() >= MAX_LOG_FILES {
        let old_idx = log_indices.remove(0);
        let _ = std::fs::remove_file(
            log_dir.join(format!("frostbar.log-{old_idx}")),
        );
    }

    let next_idx = log_indices.last().map(|n| n + 1).unwrap_or(0);
    let logfile_name = format!("frostbar.log-{next_idx}");
    let logfile_path = log_dir.join(&logfile_name);

    let logfile = tracing_appender::rolling::never(&log_dir, &logfile_name);

    let layer = fmt::layer()
        .compact()
        .with_ansi(false)
        .with_writer(logfile)
        .boxed();

    if let Err(e) = handle.modify(|l| *l = Some(layer)) {
        eprintln!("failed to reload file logger: {e}");
    }

    logfile_path
}

pub fn get_default_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "info,frostbar=debug".into()
        } else {
            "error,frostbar=info".into()
        }
    })
}

pub fn notification(msg: &str) {
    if let Err(e) = Notification::new().summary(BAR_NAMESPACE).body(msg).show()
    {
        warn!("Failed to send notification: {e:?}");
    }
}

use std::{fs, path::PathBuf};

use chrono::{DateTime, Duration, Utc};
use notify_rust::Notification;
use tracing::warn;
use tracing_subscriber::{EnvFilter, Layer};
use tracing_subscriber::{
    fmt::{self},
    registry::LookupSpan,
    reload,
};

use crate::BAR_NAMESPACE;

type BoxedLayer<S> =
    Box<dyn tracing_subscriber::layer::Layer<S> + Send + Sync + 'static>;
pub type LogHandle<S> = reload::Handle<Option<BoxedLayer<S>>, S>;

const MAX_LOG_FILES: usize = 15;
const MAX_LOG_AGE_DAYS: i64 = 7;

pub struct LogManager {
    pub state_dir: PathBuf,
}

impl LogManager {
    pub fn init() -> Self {
        let state_dir = std::env::var("XDG_STATE_HOME")
            .map(|state| PathBuf::from(state))
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").expect("$HOME should be set");
                PathBuf::from(home).join(".local/state").join(BAR_NAMESPACE)
            });

        let _ = fs::create_dir_all(&state_dir);
        Self { state_dir }
    }

    pub fn generate_log_name() -> String {
        let pid = std::process::id();
        let now = Utc::now().format("%Y%M%d-%H%M%S");
        format!("{}.{}.{}.log", BAR_NAMESPACE, pid, now)
    }

    pub fn setup_logging<S>(&self, handle: &LogHandle<S>) -> PathBuf
    where
        S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    {
        self.cleanup_old_logs();
        let log_name = Self::generate_log_name();
        let log_path = self.state_dir.join(&log_name);

        let file_appender =
            tracing_appender::rolling::never(&self.state_dir, &log_name);

        let layer = fmt::layer().compact().with_writer(file_appender).boxed();

        let _ = handle.modify(|l| *l = Some(layer));
        log_path
    }

    fn cleanup_old_logs(&self) {
        let Ok(entries) = fs::read_dir(&self.state_dir) else {
            return;
        };
        let now = Utc::now();
        let expiration = Duration::days(MAX_LOG_AGE_DAYS);

        let mut log_files: Vec<(PathBuf, DateTime<Utc>)> = entries
            .flatten()
            .filter(|e| {
                e.file_name().to_string_lossy().starts_with(BAR_NAMESPACE)
            })
            .filter_map(|e| {
                let path = e.path();
                let meta = e.metadata().ok()?;
                let modified = meta.modified().ok()?.into();
                Some((path, modified))
            })
            .collect();

        log_files.retain(|(path, modified)| {
            if now.signed_duration_since(*modified) > expiration {
                let _ = fs::remove_file(path);
                false
            } else {
                true
            }
        });

        if log_files.len() > MAX_LOG_FILES {
            log_files.sort_by_key(|&(_, modified)| modified);
            let to_remove = log_files.len() - MAX_LOG_FILES;
            for i in 0..to_remove {
                let _ = fs::remove_file(&log_files[i].0);
            }
        }
    }

    pub fn find_log(&self, pid: Option<u32>) -> Option<PathBuf> {
        let entries = fs::read_dir(&self.state_dir).ok()?;
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "log"))
            .collect();

        if let Some(target_pid) = pid {
            let pid_str = format!(".{}.", target_pid);
            files
                .into_iter()
                .find(|p| p.to_string_lossy().contains(&pid_str))
        } else {
            files.sort_by_key(|p| {
                fs::metadata(p).and_then(|m| m.modified()).ok()
            });
            files.last().cloned()
        }
    }
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

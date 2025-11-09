use std::path::{Path, PathBuf};
use tracing::{Level, error};
use tracing_subscriber::{
    fmt::{self, time::ChronoLocal, writer::MakeWriterExt},
    registry::LookupSpan,
    reload,
};

pub const TIME_FORMAT_STRING: &'static str = "%m-%d %H:%M:%S%.3f";

type BoxedLayer<S> =
    Box<dyn tracing_subscriber::layer::Layer<S> + Send + Sync + 'static>;

const MAX_LOG_FILES: usize = 10;
pub fn init_tracing<S: tracing::Subscriber + for<'a> LookupSpan<'a>>(
    config_dir: &Path,
    handle: &reload::Handle<Option<BoxedLayer<S>>, S>,
) -> PathBuf {
    let log_dir = config_dir.join("logs/");
    let mut nlog = 0;
    let mut min_log = u64::MAX;

    match read_log_dir(&log_dir) {
        Ok(log_files) => {
            let mut num_files = 0;
            for filename in log_files {
                num_files += 1;
                let trailing = filename
                    .rsplit_once(|c: char| !c.is_ascii_digit())
                    .map_or(filename.as_str(), |(_head, digits)| digits);

                if let Ok(trailing_digits) = trailing.parse::<u64>() {
                    if trailing_digits > nlog {
                        nlog = trailing_digits;
                    }
                    if trailing_digits < min_log {
                        min_log = trailing_digits;
                    }
                }
            }
            if num_files >= MAX_LOG_FILES
                && let Err(e) = std::fs::remove_file(
                    log_dir.join(format!("frostbar.log-{min_log}")),
                )
            {
                error!("failed to remove old log file: {e}");
            }
            nlog += 1;
        }
        Err(e) => {
            error!("failed to read log directory: {e}");
        }
    }

    let logfile_path = log_dir.join(format!("frostbar.log-{nlog}"));

    let logfile = tracing_appender::rolling::never(
        &log_dir,
        logfile_path.file_name().unwrap(),
    )
    .with_max_level(Level::INFO);

    let logfile_layer = fmt::layer()
        .compact()
        .with_writer(logfile)
        .with_ansi(false)
        .with_timer(ChronoLocal::new(TIME_FORMAT_STRING.to_string()));

    let boxed_layer: BoxedLayer<S> = Box::new(logfile_layer);

    if let Err(e) = handle.modify(|layer| *layer = Some(boxed_layer)) {
        tracing::error!("Failed to reload file logging layer: {}", e);
    }

    logfile_path
}

fn read_log_dir(path: &Path) -> std::io::Result<Vec<String>> {
    let mut filenames = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            filenames.push(name.to_string());
        }
    }
    Ok(filenames)
}

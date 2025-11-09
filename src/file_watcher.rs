// This file was adapted from src/utils/watcher.rs in niri
// (https://github.com/YaLTeR/niri/blob/271534e115e5915231c99df287bbfe396185924d/src/utils/watcher.rs)
//
// niri is licensed under the GNU General Public License v3.0 (GPL-3.0).

use crate::Message;
use iced::{
    Subscription,
    advanced::subscription::{EventStream, Recipe, from_recipe},
};
use std::{
    hash::Hash,
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

const POLLING_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
pub struct ConfigPath {
    pub config: PathBuf,
    pub colors: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub config: CheckType,
    pub colors: CheckType,
}

impl Default for CheckResult {
    fn default() -> Self {
        Self {
            config: CheckType::Unchanged,
            colors: CheckType::Unchanged,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckType {
    Changed,
    Missing,
    Unchanged,
}

struct FileWatcherProps {
    config: Option<(SystemTime, PathBuf)>,
    colors: Option<(SystemTime, PathBuf)>,
}

struct FileWatcherInner {
    path: ConfigPath,
    props: FileWatcherProps,
}

#[profiling::all_functions]
impl FileWatcherInner {
    pub fn new(path: ConfigPath) -> Self {
        Self {
            props: FileWatcherProps {
                config: see_path(&path.config).ok(),
                colors: see_path(&path.colors).ok(),
            },
            path,
        }
    }

    pub fn check(&mut self) -> CheckResult {
        let mut result = CheckResult::default();
        if let Ok(new_props) = see_path(&self.path.colors) {
            if self.props.colors.as_ref() != Some(&new_props) {
                self.props.colors = Some(new_props);
                result.colors = CheckType::Changed;
            }
            // unchanged
        } else {
            result.colors = CheckType::Missing;
        }

        if let Ok(new_props) = see_path(&self.path.config) {
            if self.props.config.as_ref() != Some(&new_props) {
                self.props.config = Some(new_props);
                result.config = CheckType::Changed;
            }
            // unchanged
        } else {
            result.config = CheckType::Missing;
        }

        result
    }
}

struct FileWatcher {
    path: ConfigPath,
}

impl FileWatcher {
    fn new(path: ConfigPath) -> Self {
        Self { path }
    }
}

#[profiling::all_functions]
impl Recipe for FileWatcher {
    type Output = CheckResult;

    fn hash(&self, state: &mut iced::advanced::subscription::Hasher) {
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: EventStream,
    ) -> iced::futures::stream::BoxStream<'static, Self::Output> {
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            profiling::register_thread!("file watcher");
            let mut watcher = FileWatcherInner::new(self.path);
            loop {
                tokio::time::sleep(POLLING_INTERVAL).await;

                let event = watcher.check();

                if tx.send(event).is_err() {
                    break;
                }
            }
        });

        Box::pin(UnboundedReceiverStream::new(rx))
    }
}

pub fn watch_config(path: ConfigPath) -> Subscription<Message> {
    from_recipe(FileWatcher::new(path)).map(Message::FileWatcherEvent)
}

fn see_path(path: &Path) -> io::Result<(SystemTime, PathBuf)> {
    let canon = path.canonicalize()?;
    let mtime = canon.metadata()?.modified()?;
    Ok((mtime, canon))
}

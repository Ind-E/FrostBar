// This file was adapted from src/utils/watcher.rs in niri
// (https://github.com/YaLTeR/niri/blob/271534e115e5915231c99df287bbfe396185924d/src/utils/watcher.rs)
//
// niri is licensed under the GNU General Public License v3.0 (GPL-3.0).

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

use crate::{Message, utils::BoxStream};

const POLLING_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
pub enum FileWatcherEvent {
    Changed,
    Missing,
}

pub struct FileWatcherInner {
    path: PathBuf,

    last_props: Option<(SystemTime, PathBuf)>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CheckResult {
    Missing,
    Unchanged,
    Changed,
}

struct FileWatcher {
    path: PathBuf,
}

impl FileWatcher {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[profiling::all_functions]
impl Recipe for FileWatcher {
    type Output = FileWatcherEvent;

    fn hash(&self, state: &mut iced::advanced::subscription::Hasher) {
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<Self::Output> {
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut watcher = FileWatcherInner::new(self.path);
            loop {
                tokio::time::sleep(POLLING_INTERVAL).await;

                let event = match watcher.check() {
                    CheckResult::Changed => Some(FileWatcherEvent::Changed),
                    CheckResult::Missing => Some(FileWatcherEvent::Missing),
                    CheckResult::Unchanged => None,
                };

                if let Some(event) = event
                    && tx.send(event).is_err()
                {
                    break;
                }
            }
        });

        Box::pin(UnboundedReceiverStream::new(rx))
    }
}

pub fn watch_file(path: PathBuf) -> Subscription<Message> {
    from_recipe(FileWatcher::new(path)).map(Message::FileWatcherEvent)
}

fn see_path(path: &Path) -> io::Result<(SystemTime, PathBuf)> {
    let canon = path.canonicalize()?;
    let mtime = canon.metadata()?.modified()?;
    Ok((mtime, canon))
}


#[profiling::all_functions]
impl FileWatcherInner {
    pub fn new(path: PathBuf) -> Self {
        let last_props = see_path(&path).ok();
        Self { path, last_props }
    }

    pub fn check(&mut self) -> CheckResult {
        if let Ok(new_props) = see_path(&self.path) {
            if self.last_props.as_ref() == Some(&new_props) {
                CheckResult::Unchanged
            } else {
                self.last_props = Some(new_props);
                CheckResult::Changed
            }
        } else {
            CheckResult::Missing
        }
    }
}

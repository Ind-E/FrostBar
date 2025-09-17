// This file was adapted from src/utils/watcher.rs in niri
// (https://github.com/YaLTeR/niri/blob/271534e115e5915231c99df287bbfe396185924d/src/utils/watcher.rs)
//
// niri is licensed under the GNU General Public License v3.0 (GPL-3.0).

use iced::Subscription;
use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use crate::Message;

const POLLING_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
pub enum FileWatcherEvent {
    Changed,
    Missing,
}

pub struct FileWatcher {
    path: PathBuf,

    last_props: Option<(SystemTime, PathBuf)>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CheckResult {
    Missing,
    Unchanged,
    Changed,
}

pub fn watch_file(path: PathBuf) -> Subscription<Message> {
    Subscription::run_with(path, |path| {
        let path = path.clone();
        async_stream::stream! {
            let mut watcher = FileWatcher::new(path);

            loop {
                tokio::time::sleep(POLLING_INTERVAL).await;

                match watcher.check() {
                    CheckResult::Changed => yield FileWatcherEvent::Changed,
                    CheckResult::Missing => yield FileWatcherEvent::Missing,
                    CheckResult::Unchanged => {}

                }
            }

        }
    })
    .map(Message::FileWatcherEvent)
}

fn see_path(path: &Path) -> io::Result<(SystemTime, PathBuf)> {
    let canon = path.canonicalize()?;
    let mtime = canon.metadata()?.modified()?;
    Ok((mtime, canon))
}

impl FileWatcher {
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

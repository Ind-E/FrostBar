// This file was adapted from src/utils/watcher.rs in niri
// (https://github.com/YaLTeR/niri/blob/271534e115e5915231c99df287bbfe396185924d/src/utils/watcher.rs)
//
// niri is licensed under the GNU General Public License v3.0 (GPL-3.0).

use std::{
    hash::Hash,
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use iced::{Subscription, futures::channel::mpsc::Sender};

use crate::Message;

const POLLING_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Hash)]
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
    Disappeared,
    Missing,
    Unchanged,
}

#[derive(Hash)]
struct FileWatcherProps {
    config: Option<(SystemTime, PathBuf)>,
    colors: Option<(SystemTime, PathBuf)>,
}

struct FileWatcher<'a> {
    path: &'a ConfigPath,
    props: FileWatcherProps,
}

#[profiling::all_functions]
impl<'a> FileWatcher<'a> {
    pub fn new(path: &'a ConfigPath) -> Self {
        Self {
            props: FileWatcherProps {
                config: see_path(&path.config).ok(),
                colors: see_path(&path.colors).ok(),
            },
            path,
        }
    }

    fn check_file(
        path: &Path,
        state: &mut Option<(SystemTime, PathBuf)>,
    ) -> CheckType {
        match see_path(path) {
            Ok(new_props) => {
                if state.as_ref() == Some(&new_props) {
                    CheckType::Unchanged
                } else {
                    *state = Some(new_props);
                    CheckType::Changed
                }
            }
            Err(_) => {
                if state.is_some() {
                    *state = None;
                    CheckType::Disappeared
                } else {
                    CheckType::Missing
                }
            }
        }
    }

    pub fn check(&mut self) -> CheckResult {
        CheckResult {
            config: Self::check_file(&self.path.config, &mut self.props.config),
            colors: Self::check_file(&self.path.colors, &mut self.props.colors),
        }
    }
}

pub fn watch_config(path: ConfigPath) -> Subscription<Message> {
    Subscription::run_with(path, move |path| {
        let path = path.clone();
        iced::stream::channel(
            100,
            |mut output: Sender<CheckResult>| async move {
                let mut watcher = FileWatcher::new(&path);
                loop {
                    tokio::time::sleep(POLLING_INTERVAL).await;

                    let event = watcher.check();

                    if (event.config != CheckType::Unchanged
                        || event.colors != CheckType::Unchanged)
                        && output.try_send(event).is_err()
                    {
                        break;
                    }
                }
            },
        )
    })
    .map(Message::FileWatcherEvent)
}

fn see_path(path: &Path) -> io::Result<(SystemTime, PathBuf)> {
    let canon = path.canonicalize()?;
    let mtime = canon.metadata()?.modified()?;
    Ok((mtime, canon))
}

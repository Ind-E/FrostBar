use iced::{
    Color,
    advanced::subscription::{EventStream, Recipe, from_recipe},
};
use std::{
    env::temp_dir,
    fs,
    hash::Hash,
    io::{self, BufRead},
    process::{Command, Stdio},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::error;

use crate::{Message, services::Service, utils::BoxStream};

const CAVA_CONFIG: &str = include_str!("../../assets/cava-config");

pub fn write_temp_cava_config() -> std::io::Result<std::path::PathBuf> {
    let tmp_path = temp_dir().join("my_cava_config");
    fs::write(&tmp_path, CAVA_CONFIG)?;
    Ok(tmp_path)
}

fn default_gradient() -> Vec<Color> {
    (0..20)
        .map(|i| {
            let intensity = 0.8 + (i as f32 / 20.0) * 0.2;
            Color::from_rgb(intensity, intensity, intensity)
        })
        .collect()
}

pub struct CavaService {
    pub bars: Vec<u8>,
    pub colors: Vec<Color>,
}

struct CavaSubscriptionRecipe {}

#[profiling::all_functions]
impl Recipe for CavaSubscriptionRecipe {
    type Output = Option<String>;

    fn hash(&self, state: &mut iced::advanced::subscription::Hasher) {
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<Self::Output> {
        let (tx, rx) = mpsc::channel::<Option<String>>(128);

        let config_path =
            write_temp_cava_config().unwrap().display().to_string();

        tokio::task::spawn_blocking(move || {
            let mut command = match Command::new("cava")
                .arg("-p")
                .arg(&config_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(cmd) => cmd,
                Err(e) => {
                    error!("Cava Command Failed: {e}");
                    return;
                }
            };

            let Some(stdout) = command.stdout.take() else {
                error!("Cava Pipe Failed");
                return;
            };

            let reader = io::BufReader::new(stdout);

            for line in reader.lines() {
                match line {
                    Ok(line_str) => {
                        if tx.blocking_send(Some(line_str)).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            let _ = command.kill();
        });

        Box::pin(ReceiverStream::new(rx))
    }
}

#[profiling::all_functions]
impl Service for CavaService {
    fn subscription() -> iced::Subscription<Message> {
        from_recipe(CavaSubscriptionRecipe {}).map(Message::CavaUpdate)
    }

    type Event = Option<String>;
    fn handle_event(&mut self, event: Self::Event) -> iced::Task<Message> {
        if let Some(line) = event {
            self.bars = line
                .split(';')
                .map(|s| s.parse::<u8>().unwrap_or(0))
                .collect();
        }
        iced::Task::none()
    }
}

impl CavaService {
    pub fn new() -> Self {
        Self {
            bars: vec![],
            colors: default_gradient(),
        }
    }

    pub fn update_gradient(
        &mut self,
        colors: Option<Vec<Color>>,
    ) -> iced::Task<Message> {
        self.colors = colors.unwrap_or_else(default_gradient);
        iced::Task::none()
    }
}

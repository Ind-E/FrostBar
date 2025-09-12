use iced::{Color, Subscription};
use std::{
    env::temp_dir,
    fs,
    io::{self, BufRead},
    process::{Command, Stdio},
    thread,
};
use tracing::error;

use crate::{Message, services::Service};

#[derive(Debug, Clone, thiserror::Error)]
pub enum CavaError {
    #[error("Cava command failed to start: {0}")]
    CommandFailed(String),
    #[error("Could not capture Cava's stdout pipe")]
    PipeFailed,
}

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

impl Service for CavaService {
    fn subscription() -> iced::Subscription<Message> {
        Subscription::run(|| {
            async_stream::stream! {
                let (tx, rx) = async_channel::unbounded::<Result<String, CavaError>>();

                let config_path = write_temp_cava_config().unwrap().display().to_string();

                thread::spawn(move || {
                    let mut command = match Command::new("cava")
                        .arg("-p")
                        .arg(&config_path)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .spawn()
                    {
                        Ok(cmd) => cmd,
                        Err(e) => {
                            let _ = tx.send_blocking(Err(CavaError::CommandFailed(e.to_string())));
                            return;
                        }
                    };

                    let stdout = match command.stdout.take() {
                        Some(pipe) => pipe,
                        Option::None => {
                            let _ = tx.send_blocking(Err(CavaError::PipeFailed));
                            return;
                        }
                    };

                    let reader = io::BufReader::new(stdout);

                    for line in reader.lines() {
                        match line {
                            Ok(line_str) => {
                                if tx.send_blocking(Ok(line_str)).is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }

                    let _ = command.kill();
                });

                while let Ok(result) = rx.recv().await {
                    yield result;
                }
            }
        }).map(Message::CavaUpdate)
    }

    type Event = Result<String, CavaError>;
    fn handle_event(&mut self, event: Self::Event) -> iced::Task<Message> {
        match event {
            Ok(line) => {
                self.bars = line
                    .split(";")
                    .map(|s| s.parse::<u8>().unwrap_or(0))
                    .collect();
            }
            Err(e) => {
                error!("cava error: {e}");
            }
        };
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

    pub fn update_gradient(&mut self, colors: Option<Vec<Color>>) -> iced::Task<Message> {
        self.colors = colors.unwrap_or_else(default_gradient);
        iced::Task::none()
    }
}

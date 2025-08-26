use iced::{
    Color, Element, Length, Point, Renderer, Size,
    advanced::subscription,
    futures::Stream,
    mouse::ScrollDelta,
    widget::{
        Canvas, MouseArea,
        canvas::{Cache, Frame, Geometry, Program},
    },
};
use std::{
    env::temp_dir,
    fs,
    hash::Hash,
    io::{self, BufRead},
    pin::Pin,
    process::{Command, Stdio},
    thread,
};
use tracing::error;

use crate::{Message, config::Cava};

#[derive(Debug, Clone, thiserror::Error)]
pub enum CavaError {
    #[error("Cava command failed to start: {0}")]
    CommandFailed(String),
    #[error("Could not capture Cava's stdout pipe")]
    PipeFailed,
}

const CAVA_CONFIG: &str = include_str!("../../assets/cava-config");
const MAX_BAR_HEIGHT: u32 = 12;

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

pub struct CavaModule {
    bars: Vec<u8>,
    cache: Cache,
    colors: Vec<Color>,
    config: Cava,
}

impl CavaModule {
    pub fn new(config: Cava) -> Self {
        Self {
            bars: vec![0; config.bars],
            config,
            cache: Cache::new(),
            colors: default_gradient(),
        }
    }

    pub fn update_gradient(&mut self, colors: Option<Vec<Color>>) {
        self.colors = colors.unwrap_or_else(default_gradient);
        self.cache.clear();
    }

    pub fn update(&mut self, update: Result<String, CavaError>) -> iced::Task<Message> {
        match update {
            Ok(line) => {
                self.bars = line
                    .split(";")
                    .map(|s| s.parse::<u8>().unwrap_or(0))
                    .collect();
                self.cache.clear();
            }
            Err(e) => {
                error!("cava error: {e}");
            }
        };
        iced::Task::none()
    }

    pub fn to_widget<'a>(&'a self) -> Element<'a, Message> {
        MouseArea::new(Canvas::new(self).width(Length::Fill).height(130))
            .on_scroll(|delta| {
                Message::ChangeVolume(match delta {
                    ScrollDelta::Lines { x, y } => {
                        if y > 0.0 || x < 0.0 {
                            self.config.volume_percent
                        } else {
                            -self.config.volume_percent
                        }
                    }
                    ScrollDelta::Pixels { x, y } => {
                        if y > 0.0 || x < 0.0 {
                            self.config.volume_percent
                        } else {
                            -self.config.volume_percent
                        }
                    }
                })
            })
            .on_press(Message::Command("pavucontrol".to_string()))
            .on_right_press(Message::Command("blueman-manager".to_string()))
            .into()
    }
}

impl<Message> Program<Message> for CavaModule {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let bars = self
            .cache
            .draw(renderer, bounds.size(), |frame: &mut Frame| {
                let center_x = frame.center().x;

                let bars_per_channel = self.bars.len() / 2;

                if bars_per_channel == 0 {
                    return;
                }

                let bar_thickness_total = frame.height() / bars_per_channel as f32;
                let spacing = bar_thickness_total * self.config.spacing;
                let bar_thickness = bar_thickness_total - spacing;

                for i in 0..bars_per_channel {
                    let left_val = self.bars[i];
                    let right_val = self.bars[2 * bars_per_channel - i - 1];

                    let max_bar_width = center_x;
                    let left_width =
                        max_bar_width * (left_val as f32 / MAX_BAR_HEIGHT as f32);
                    let right_width =
                        max_bar_width * (right_val as f32 / MAX_BAR_HEIGHT as f32);

                    let y_pos = i as f32 * bar_thickness_total + spacing / 2.0;

                    let color_index = (i * self.colors.len()) / bars_per_channel;

                    let bar_color = self.colors.get(color_index).unwrap_or(&Color::WHITE);

                    if left_val > 0 {
                        let top_left = Point {
                            x: center_x - left_width,
                            y: y_pos,
                        };
                        let bar_size = Size::new(left_width, bar_thickness);
                        frame.fill_rectangle(top_left, bar_size, *bar_color);
                    }

                    if right_val > 0 {
                        let top_left = Point {
                            x: center_x,
                            y: y_pos,
                        };
                        let bar_size = Size::new(right_width, bar_thickness);
                        frame.fill_rectangle(top_left, bar_size, *bar_color);
                    }
                }
            });

        vec![bars]
    }
}

#[derive(Debug, Clone)]
pub struct CavaEvents {
    pub config_path: String,
}

impl subscription::Recipe for CavaEvents {
    type Output = Result<String, CavaError>;

    fn hash(&self, state: &mut subscription::Hasher) {
        std::any::TypeId::of::<Self>().hash(state);
        self.config_path.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: subscription::EventStream,
    ) -> Pin<Box<dyn Stream<Item = Self::Output> + Send>> {
        // let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        //
        // thread::spawn(move || {
        //     let mut command = match Command::new("cava")
        //         .arg("-p")
        //         .arg(&self.config_path)
        //         .stdout(Stdio::piped())
        //         .stderr(Stdio::null())
        //         .spawn()
        //     {
        //         Ok(cmd) => cmd,
        //         Err(e) => {
        //             let _ = tx.send(Err(CavaError::CommandFailed(e.to_string())));
        //             return;
        //         }
        //     };
        //
        //     let stdout = match command.stdout.take() {
        //         Some(pipe) => pipe,
        //         None => {
        //             let _ = tx.send(Err(CavaError::PipeFailed));
        //             return;
        //         }
        //     };
        //
        //     let reader = io::BufReader::new(stdout);
        //
        //     for line in reader.lines() {
        //         match line {
        //             Ok(line_str) => {
        //                 if tx.send(Ok(line_str)).is_err() {
        //                     break;
        //                 }
        //             }
        //             Err(_) => break,
        //         }
        //     }
        //
        //     let _ = command.kill();
        // });
        //
        // Box::pin(UnboundedReceiverStream::new(rx))
        // }

        Box::pin(async_stream::stream! {
            let (tx, rx) = async_channel::unbounded::<Result<String, CavaError>>();

            thread::spawn(move || {
                let mut command = match Command::new("cava")
                    .arg("-p")
                    .arg(&self.config_path)
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
        })
    }
}

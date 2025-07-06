use iced::{
    Color, Point, Renderer, Size,
    advanced::subscription,
    widget::canvas::{Cache, Frame, Geometry, Program},
};
use std::{
    env::temp_dir,
    fs,
    hash::Hash,
    io::{self, BufRead},
    process::{Command, Stdio},
};

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

pub struct CavaVisualizer {
    pub bars: Vec<u8>,
    pub cache: Cache,
}

impl<Message> Program<Message> for CavaVisualizer {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &iced_runtime::core::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let bars =
            self.cache
                .draw(renderer, bounds.size(), |frame: &mut Frame| {
                    let left_color = Color::from_rgb(0.4, 0.9, 0.6);
                    let right_color = Color::from_rgb(0.9, 0.6, 0.4);

                    let center_x = frame.center().x;

                    let bars_per_channel = self.bars.len() / 2;

                    if bars_per_channel == 0 {
                        return;
                    }

                    let bar_thickness_total =
                        frame.height() / bars_per_channel as f32;
                    let spacing = bar_thickness_total * 0.15;
                    let bar_thickness = bar_thickness_total - spacing;

                    for i in 0..bars_per_channel {
                        let left_val = self.bars[i];
                        let right_val = self.bars[2 * bars_per_channel - i - 1];

                        let max_bar_width = center_x;
                        let left_width = max_bar_width
                            * (left_val as f32 / MAX_BAR_HEIGHT as f32);
                        let right_width = max_bar_width
                            * (right_val as f32 / MAX_BAR_HEIGHT as f32);

                        let y_pos =
                            i as f32 * bar_thickness_total + spacing / 2.0;

                        if left_val > 0 {
                            let top_left = Point {
                                x: center_x - left_width,
                                y: y_pos,
                            };
                            let bar_size = Size::new(left_width, bar_thickness);
                            frame.fill_rectangle(top_left, bar_size, left_color);
                        }

                        if right_val > 0 {
                            let top_left = Point {
                                x: center_x,
                                y: y_pos,
                            };
                            let bar_size = Size::new(right_width, bar_thickness);
                            frame.fill_rectangle(top_left, bar_size, right_color);
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
    ) -> iced_runtime::futures::BoxStream<Self::Output> {
        Box::pin(async_stream::stream! {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

            tokio::task::spawn_blocking(move || {
                let mut command = match Command::new("cava")
                    .arg("-p")
                    .arg(&self.config_path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(cmd) => cmd,
                    Err(e) => {
                        let _ = tx.send(Err(CavaError::CommandFailed(e.to_string())));
                        return;
                    }
                };

                let stdout = match command.stdout.take() {
                    Some(pipe) => pipe,
                    Option::None => {
                        let _ = tx.send(Err(CavaError::PipeFailed));
                        return;
                    }
                };

                let reader = io::BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(line_str) => {
                            if tx.send(Ok(line_str)).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }

                let _ = command.kill();
            });

            while let Some(result) = rx.recv().await {
                yield result;
            }
        })
    }
}

use crate::{
    Element, config,
    modules::{BarPosition, Modules, ViewTrait, mouse_binds},
    utils::style::container_style,
};
use iced::{
    Length, Point, Renderer, Size,
    widget::{Canvas, Container, canvas},
};
use std::any::Any;

use super::service::AudioVisualizerService;

pub struct AudioVisualizerView {
    config: config::AudioVisualizer,
    pub position: BarPosition,
}

impl AudioVisualizerView {
    pub fn new(config: config::AudioVisualizer, position: BarPosition) -> Self {
        Self { config, position }
    }
}

#[profiling::all_functions]
impl ViewTrait<Modules> for AudioVisualizerView {
    fn view<'a>(
        &'a self,
        modules: &'a Modules,
        layout: &'a config::Layout,
    ) -> Element<'a> {
        let audio = &modules.audio_visualizer;
        let vertical = layout.anchor.vertical();
        let canvas = if vertical {
            Canvas::new(AudioVisualizerCanvas::new(audio, &self.config, vertical))
                .width(Length::Fill)
                .height(130)
        } else {
            Canvas::new(AudioVisualizerCanvas::new(audio, &self.config, vertical))
                .width(130)
                .height(Length::Fill)
        };

        let container =
            container_style(Container::new(canvas), &self.config.style, layout);

        mouse_binds(container, &self.config.binds, None)
    }

    fn position(&self) -> BarPosition {
        self.position
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct AudioVisualizerCanvas<'a> {
    service: &'a AudioVisualizerService,
    config: &'a config::AudioVisualizer,
    cache: canvas::Cache,
    vertical: bool,
}

impl<'a> AudioVisualizerCanvas<'a> {
    pub fn new(
        service: &'a AudioVisualizerService,
        config: &'a config::AudioVisualizer,
        vertical: bool,
    ) -> Self {
        Self {
            service,
            config,
            cache: canvas::Cache::new(),
            vertical,
        }
    }
}

#[profiling::all_functions]
impl<Message> canvas::Program<Message> for AudioVisualizerCanvas<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let bars = self.cache.draw(
            renderer,
            bounds.size(),
            |frame: &mut canvas::Frame| {
                let center = frame.center();

                let bars = &self.service.bars;

                let bars_per_channel = bars.len() / 2;

                if bars_per_channel == 0 {
                    return;
                }

                let bar_thickness_total = if self.vertical {
                    frame.height() / bars_per_channel as f32
                } else {
                    frame.width() / bars_per_channel as f32
                };
                let spacing = bar_thickness_total * self.config.spacing;
                let bar_thickness = bar_thickness_total - spacing;

                for i in 0..bars_per_channel {
                    let right_val = bars[bars_per_channel - i - 1];
                    let left_val = bars[2 * bars_per_channel - i - 1];

                    let max_bar_width =
                        if self.vertical { center.x } else { center.y };

                    let left_width = max_bar_width * left_val * 2.0;
                    let right_width = max_bar_width * right_val * 2.0;

                    let pos = i as f32 * bar_thickness_total + spacing / 2.0;

                    let bar_color = if self.config.dynamic_color {
                        self.service.gradient.as_ref().and_then(|gradient| {
                            gradient.get(i * gradient.len() / bars_per_channel)
                        })
                    } else {
                        None
                    }
                    .unwrap_or(&self.config.color);

                    if left_val > 0.0 {
                        let (top_left, bar_size) = if self.vertical {
                            (
                                Point {
                                    x: center.x - left_width,
                                    y: pos,
                                },
                                Size::new(left_width, bar_thickness),
                            )
                        } else {
                            (
                                Point {
                                    x: pos,
                                    y: center.y - left_width,
                                },
                                Size::new(bar_thickness, left_width),
                            )
                        };
                        frame.fill_rectangle(top_left, bar_size, *bar_color);
                    }

                    if right_val > 0.0 {
                        let (top_left, bar_size) = if self.vertical {
                            (
                                Point {
                                    x: center.x,
                                    y: pos,
                                },
                                Size::new(right_width, bar_thickness),
                            )
                        } else {
                            (
                                Point {
                                    x: pos,
                                    y: center.y,
                                },
                                Size::new(bar_thickness, right_width),
                            )
                        };
                        frame.fill_rectangle(top_left, bar_size, *bar_color);
                    }
                }
            },
        );

        vec![bars]
    }
}

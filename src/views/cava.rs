use iced::{
    Element, Length, Point, Renderer, Size,
    widget::{Canvas, Container, canvas},
};

use crate::{
    Message, config,
    module::Modules,
    services::cava::CavaService,
    style::container_style,
    utils::mouse_binds,
    views::{BarPosition, ViewTrait},
};

const MAX_BAR_HEIGHT: u32 = 12;

pub struct CavaView {
    config: config::Cava,
    pub position: BarPosition,
}

impl CavaView {
    pub fn new(config: config::Cava, position: BarPosition) -> Self {
        Self { config, position }
    }
}

#[profiling::all_functions]
impl ViewTrait<Modules> for CavaView {
    fn view<'a>(
        &'a self,
        modules: &'a Modules,
        layout: &'a config::Layout,
    ) -> Element<'a, Message> {
        let cava = &modules.cava;
        let vertical = layout.anchor.vertical();
        let canvas = if vertical {
            Canvas::new(CavaCanvas::new(cava, &self.config, vertical))
                .width(Length::Fill)
                .height(130)
        } else {
            Canvas::new(CavaCanvas::new(cava, &self.config, vertical))
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
}

struct CavaCanvas<'a> {
    service: &'a CavaService,
    config: &'a config::Cava,
    cache: canvas::Cache,
    vertical: bool,
}

impl<'a> CavaCanvas<'a> {
    pub fn new(
        service: &'a CavaService,
        config: &'a config::Cava,
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

impl<Message> canvas::Program<Message> for CavaCanvas<'_> {
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

                let bars_per_channel = self.service.bars.len() / 2;

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
                    let left_val = self.service.bars[i];
                    let right_val =
                        self.service.bars[2 * bars_per_channel - i - 1];

                    let max_bar_width =
                        if self.vertical { center.x } else { center.y };
                    let left_width = max_bar_width
                        * (f32::from(left_val) / MAX_BAR_HEIGHT as f32);
                    let right_width = max_bar_width
                        * (f32::from(right_val) / MAX_BAR_HEIGHT as f32);

                    let pos = i as f32 * bar_thickness_total + spacing / 2.0;

                    let bar_color = if self.config.dynamic_color {
                        self.service.gradient.as_ref().and_then(|gradient| {
                            gradient.get(i * gradient.len() / bars_per_channel)
                        })
                    } else {
                        None
                    }
                    .unwrap_or(&self.config.color);

                    if left_val > 0 {
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

                    if right_val > 0 {
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

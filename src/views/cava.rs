use iced::{
    Color, Element, Length, Point, Renderer, Size,
    mouse::ScrollDelta,
    widget::{Canvas, MouseArea, canvas},
};

use crate::{Message, config, services::cava::CavaService, views::BarPosition};

const MAX_BAR_HEIGHT: u32 = 12;

pub struct CavaView {
    config: config::Cava,
    position: BarPosition,
}

impl CavaView {
    pub fn new(config: config::Cava, position: BarPosition) -> Self {
        Self { config, position }
    }
}

impl<'a> CavaView {
    pub fn view(&'a self, service: &'a CavaService) -> Element<'a, Message> {
        MouseArea::new(
            Canvas::new(CavaCanvas::new(service, &self.config))
                .width(Length::Fill)
                .height(130),
        )
        .on_scroll(|delta| {
            Message::ChangeVolume({
                let (x, y) = match delta {
                    ScrollDelta::Lines { x, y } | ScrollDelta::Pixels { x, y } => (x, y),
                };

                if y > 0.0 || x < 0.0 {
                    self.config.volume_percent
                } else {
                    -self.config.volume_percent
                }
            })
        })
        .on_press(Message::Command("pavucontrol".to_string()))
        .on_right_press(Message::Command("blueman-manager".to_string()))
        .into()
    }
}

struct CavaCanvas<'a> {
    service: &'a CavaService,
    config: &'a config::Cava,
    cache: canvas::Cache,
}

impl<'a> CavaCanvas<'a> {
    pub fn new(service: &'a CavaService, config: &'a config::Cava) -> Self {
        Self {
            service,
            config,
            cache: canvas::Cache::new(),
        }
    }
}

impl<'a, Message> canvas::Program<Message> for CavaCanvas<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let bars =
            self.cache
                .draw(renderer, bounds.size(), |frame: &mut canvas::Frame| {
                    let center_x = frame.center().x;

                    let bars_per_channel = self.service.bars.len() / 2;

                    if bars_per_channel == 0 {
                        return;
                    }

                    let bar_thickness_total = frame.height() / bars_per_channel as f32;
                    let spacing = bar_thickness_total * self.config.spacing;
                    let bar_thickness = bar_thickness_total - spacing;

                    for i in 0..bars_per_channel {
                        let left_val = self.service.bars[i];
                        let right_val = self.service.bars[2 * bars_per_channel - i - 1];

                        let max_bar_width = center_x;
                        let left_width =
                            max_bar_width * (left_val as f32 / MAX_BAR_HEIGHT as f32);
                        let right_width =
                            max_bar_width * (right_val as f32 / MAX_BAR_HEIGHT as f32);

                        let y_pos = i as f32 * bar_thickness_total + spacing / 2.0;

                        let color_index =
                            (i * self.service.colors.len()) / bars_per_channel;

                        let bar_color = self
                            .service
                            .colors
                            .get(color_index)
                            .unwrap_or(&Color::WHITE);

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

use std::any::Any;

use iced::{
    Alignment,
    widget::{Column, Image, Stack, Svg},
};

use crate::{
    Message, config,
    icon_cache::Icon,
    module::Modules,
    views::{BarPosition, ViewTrait},
};

pub struct SystrayView {
    position: BarPosition,
}

impl SystrayView {
    pub fn new(position: BarPosition) -> Self {
        Self { position }
    }
}

#[profiling::all_functions]
impl ViewTrait<Modules> for SystrayView {
    fn view<'a>(
        &'a self,
        modules: &'a Modules,
        layout: &'a config::Layout,
    ) -> iced::Element<'a, Message> {
        let icon_size = layout.width as f32 * 0.6;
        let overlay_size = icon_size * 0.35;
        let mut column = Column::new().spacing(5).align_x(Alignment::Center);
        for (item, _) in modules.systray.inner.values() {
            let mut stack = Stack::new();

            if let Some(attention_icon) = item.attention_icon.clone() {
                match attention_icon {
                    Icon::Raster(handle) => {
                        stack = stack.push(
                            Image::new(handle)
                                .height(icon_size)
                                .width(icon_size),
                        );
                    }
                    Icon::Svg(handle) => {
                        stack = stack.push(
                            Svg::new(handle).height(icon_size).width(icon_size),
                        );
                    }
                }
            } else if let Some(icon) = item.icon.clone() {
                match icon {
                    Icon::Raster(handle) => {
                        stack = stack.push(
                            Image::new(handle)
                                .height(icon_size)
                                .width(icon_size),
                        );
                    }
                    Icon::Svg(handle) => {
                        stack = stack.push(
                            Svg::new(handle).height(icon_size).width(icon_size),
                        );
                    }
                }
            }

            if let Some(overlay_icon) = item.overlay_icon.clone() {
                match overlay_icon {
                    Icon::Raster(handle) => {
                        stack = stack.push(
                            iced::widget::pin(
                                Image::new(handle)
                                    .height(overlay_size)
                                    .width(overlay_size),
                            )
                            .x(icon_size - overlay_size),
                        );
                    }
                    Icon::Svg(handle) => {
                        stack = stack.push(
                            iced::widget::pin(
                                Svg::new(handle)
                                    .height(overlay_size)
                                    .width(overlay_size),
                            )
                            .x(icon_size - overlay_size),
                        );
                    }
                }
            }

            column = column.push(stack);
        }

        column.into()
    }

    fn position(&self) -> BarPosition {
        self.position
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

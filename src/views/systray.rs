use std::any::Any;

use iced::{
    Alignment,
    widget::{Column, Image, Svg},
};

use crate::{
    Message, config,
    icon_cache::Icon,
    module::Modules,
    views::{BarAlignment, BarPosition, ViewTrait},
};

pub struct SystrayView;

#[profiling::all_functions]
impl ViewTrait<Modules> for SystrayView {
    fn view<'a>(
        &'a self,
        modules: &'a Modules,
        layout: &'a config::Layout,
    ) -> iced::Element<'a, Message> {
        let icon_size = layout.width as f32 * 0.6;
        let mut column = Column::new().spacing(5).align_x(Alignment::Center);
        for (item, _) in modules.systray.inner.values() {
            if let Some(icon) = item.icon.clone() {
                match icon {
                    Icon::Raster(handle) => {
                        column = column.push(
                            Image::new(handle)
                                .height(icon_size)
                                .width(icon_size),
                        )
                    }
                    Icon::Svg(handle) => {
                        column = column.push(
                            Svg::new(handle).height(icon_size).width(icon_size),
                        )
                    }
                }
            }
        }

        column.into()
    }

    fn position(&self) -> BarPosition {
        BarPosition {
            idx: 0,
            align: BarAlignment::End,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

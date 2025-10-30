use iced::widget::{Column, Image, Svg};

use crate::{
    Message, config,
    icon_cache::Icon,
    module::Modules,
    views::{BarAlignment, BarPosition, ViewTrait},
};

pub struct SystrayView;

impl ViewTrait<Modules> for SystrayView {
    fn view<'a>(
        &'a self,
        modules: &'a Modules,
        _layout: &'a config::Layout,
    ) -> iced::Element<'a, Message> {
        let mut column = Column::new();
        for (item, _) in modules.systray.inner.values() {
            if let Some(icon) = item.icon.clone() {
                match icon {
                    Icon::Raster(handle) => {
                        column = column.push(Image::new(handle))
                    }
                    Icon::Svg(handle) => column = column.push(Svg::new(handle)),
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
}

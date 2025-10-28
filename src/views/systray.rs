use iced::{
    advanced::image,
    widget::{Column, Image, Svg},
};

use crate::{
    icon_cache::{Icon, IconCache},
    module::Modules,
    views::{BarAlignment, BarPosition, ViewTrait},
};

pub struct SystrayView;

impl ViewTrait<Modules> for SystrayView {
    fn view<'a>(
        &'a self,
        modules: &'a Modules,
        layout: &'a crate::config::Layout,
    ) -> iced::Element<'a, crate::Message> {
        let mut column = Column::new();
        for (item, _) in modules.systray.inner.values() {
            if let Some(icon_name) = &item.icon_name {
                let mut icon_cache = IconCache::new();
                if let Some(icon) = icon_cache.get_icon(icon_name) {
                    match icon {
                        Icon::Svg(handle) => {
                            column = column.push(Svg::new(handle))
                        }
                        Icon::Raster(handle) => {
                            column = column.push(Image::new(handle))
                        }
                    }
                }
            }
            if let Some(icon_pixmaps) = &item.icon_pixmap {
                if let Some(first_pixmap) = icon_pixmaps.first() {
                    let handle =
                        image::Handle::from_bytes(first_pixmap.pixels.clone());
                    column = column.push(Image::new(handle));
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

use std::any::Any;

use iced::{
    Alignment, Element,
    mouse::Interaction,
    widget::{
        Column, Container, Image, MouseArea, Stack, Svg, Text, container,
        text::Shaping,
    },
};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use system_tray::menu::TrayMenu;

use crate::{
    Message,
    config::{self},
    icon_cache::Icon,
    module::Modules,
    services::system_tray::TrayItem,
    views::{BarPosition, ViewTrait},
};

pub struct SystemTrayView {
    config: config::SystemTray,
    position: BarPosition,
    tray_item_views: FxHashMap<String, TrayItemView>,
}

impl SystemTrayView {
    pub fn new(config: config::SystemTray, position: BarPosition) -> Self {
        Self {
            config,
            position,
            tray_item_views: FxHashMap::default(),
        }
    }
}

#[profiling::all_functions]
impl ViewTrait<Modules> for SystemTrayView {
    fn view<'a>(
        &'a self,
        modules: &'a Modules,
        layout: &'a config::Layout,
    ) -> Element<'a, Message> {
        let tray = &modules.systray;

        if layout.anchor.vertical() {
            tray.items
                .iter()
                .sorted_unstable_by_key(|(_, (item, _))| &item.id)
                .fold(
                    Column::new().spacing(5).align_x(Alignment::Center),
                    |col, (id, (item, _))| {
                        if let Some(item_view) =
                            self.tray_item_views.get(id.as_str())
                        {
                            col.push(item_view.view(item, layout))
                        } else {
                            col
                        }
                    },
                )
                .align_x(Alignment::Center)
                .into()
        } else {
            todo!()
        }
    }

    fn position(&self) -> BarPosition {
        self.position
    }

    fn tooltip<'a>(
        &'a self,
        modules: &'a Modules,
        id: &container::Id,
    ) -> Option<Element<'a, Message>> {
        let tray = &modules.systray;
        for (item_id, view) in &self.tray_item_views {
            if view.id == *id
                && let Some(item) = tray.items.get(item_id)
            {
                return Some(view.render_tooltip(item));
            }
        }
        None
    }

    fn synchronize(&mut self, modules: &Modules) {
        let tray = &modules.systray;
        self.tray_item_views
            .retain(|id, _| tray.items.contains_key(id));

        for id in tray.items.keys() {
            self.tray_item_views
                .entry(id.clone())
                .or_insert_with(TrayItemView::new);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct TrayItemView {
    id: container::Id,
}

#[profiling::all_functions]
impl TrayItemView {
    pub fn new() -> Self {
        Self {
            id: container::Id::unique(),
        }
    }
    pub fn view<'a>(
        &self,
        item: &TrayItem,
        layout: &config::Layout,
    ) -> Element<'a, Message> {
        let icon_size = layout.width as f32 * 0.6;
        let overlay_size = icon_size * 0.35;
        let mut stack = Stack::new();

        if let Some(attention_icon) = item.attention_icon.clone() {
            match attention_icon {
                Icon::Raster(handle) => {
                    stack = stack.push(
                        Image::new(handle).height(icon_size).width(icon_size),
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
                        Image::new(handle).height(icon_size).width(icon_size),
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

        MouseArea::new(Container::new(stack).id(self.id.clone()))
            .on_enter(Message::OpenTooltip(self.id.clone()))
            .on_exit(Message::CloseTooltip(self.id.clone()))
            .interaction(Interaction::Pointer)
            .into()
    }

    pub fn render_tooltip<'a>(
        &self,
        tray_item: &'a (TrayItem, Option<TrayMenu>),
    ) -> Element<'a, Message> {
        let (item, _menu) = tray_item;
        Text::new({
            if let Some(title) = &item.title
                && !title.is_empty()
            {
                title
            } else {
                &item.id
            }
        })
        .shaping(Shaping::Advanced)
        .into()
    }
}

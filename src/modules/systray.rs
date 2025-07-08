use iced::{
    Element, Length,
    advanced::subscription,
    alignment::{Horizontal, Vertical},
    padding,
    widget::{Column, Container, Image, MouseArea, Svg, text},
};
use itertools::Itertools;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use system_tray::{
    client::{Client, Event, UpdateEvent},
    data::apply_menu_diffs,
    item::StatusNotifierItem,
    menu::TrayMenu,
};
use tokio::sync::broadcast::Receiver;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::{
    bar::Message,
    icon_cache::{Icon, IconCache},
};

const ICON_SIZE: u16 = 24;

#[derive(Debug)]
pub struct SysTrayState {
    pub items: HashMap<String, SysTrayItem>,
    pub client: Option<Arc<Client>>,
    icon_cache: Arc<Mutex<IconCache>>,
}

impl SysTrayState {
    pub async fn new(icon_cache: Arc<Mutex<IconCache>>) -> Self {
        let client_maybe = Client::new().await;
        let systray_client: Option<Client>;
        let items: HashMap<String, SysTrayItem>;
        match client_maybe {
            Ok(client) => {
                let initial_items = client.items();
                items = initial_items
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(address, (item, tray_menu))| {
                        (
                            address.to_string(),
                            SysTrayItem::new(
                                address.to_string(),
                                Box::new(item.clone()),
                                tray_menu.clone(),
                            ),
                        )
                    })
                    .collect();
                systray_client = Some(client);
            }
            Err(e) => {
                systray_client = None;
                items = HashMap::new();
                log::error!("{e}");
            }
        }
        Self {
            items,
            client: systray_client.and_then(|client| Some(Arc::new(client))),
            icon_cache,
        }
    }

    pub fn on_event(&mut self, event: Event) -> iced::Task<Message> {
        match event {
            Event::Add(address, item) => {
                self.items.insert(
                    address.clone(),
                    SysTrayItem::new(address, item, None),
                );
            }
            Event::Remove(address) => {
                self.items.remove(&address);
            }
            Event::Update(address, update_event) => {
                let item = self.items.get_mut(&address).unwrap();
                match update_event {
                    UpdateEvent::Icon {
                        icon_name,
                        icon_pixmap,
                    } => {
                        if let Some(icon_name) = icon_name {
                            item.inner.icon_name = Some(icon_name);
                        }
                        if let Some(icon_pixmap) = icon_pixmap {
                            item.inner.icon_pixmap = Some(icon_pixmap);
                        }
                    }
                    UpdateEvent::Title(title) => {
                        item.inner.title = title;
                    }
                    UpdateEvent::Tooltip(tooltip) => {
                        item.inner.tool_tip = tooltip;
                    }
                    UpdateEvent::Menu(menu) => {
                        item.menu = Some(menu);
                    }
                    UpdateEvent::MenuDiff(diffs) => {
                        if let Some(menu) = &mut item.menu {
                            apply_menu_diffs(menu, &diffs);
                        }
                    }
                    UpdateEvent::MenuConnect(name) => {
                        log::warn!("{name} connected to {address}");
                    }
                    UpdateEvent::AttentionIcon(_) => {}
                    UpdateEvent::OverlayIcon(_) => {}
                    UpdateEvent::Status(_) => {}
                };
            }
        };
        iced::Task::none()
    }

    pub fn to_widget<'a>(&self) -> Element<'a, Message> {
        let tray_items = self
            .items
            .values()
            .sorted_by(|item1, item2| item1.inner.id.cmp(&item2.inner.id))
            .map(|item| item.to_widget(&mut self.icon_cache.lock().unwrap()))
            .fold(
                Column::new().padding(padding::top(5).bottom(5)).spacing(2),
                |col, item| col.push(item),
            )
            .align_x(Horizontal::Center);

        Container::new(tray_items)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Bottom)
            .into()
    }
}

#[derive(Debug)]
pub struct SysTrayItem {
    address: String,
    inner: Box<StatusNotifierItem>,
    menu: Option<TrayMenu>,
}

impl SysTrayItem {
    fn new(
        address: String,
        item: Box<StatusNotifierItem>,
        menu: Option<TrayMenu>,
    ) -> Self {
        Self {
            address,
            inner: item,
            menu,
        }
    }

    fn to_widget<'a>(&self, icon_cache: &mut IconCache) -> Element<'a, Message> {
        let mut tray_icon = None;
        if let Some(icon) = icon_cache.get_tray_icon(&self.inner).clone() {
            tray_icon = Some(icon);
        }
        if let Some(icon) = tray_icon {
            match icon {
                Icon::Svg(handle) => MouseArea::new(
                    Svg::new(handle).width(ICON_SIZE).height(ICON_SIZE),
                )
                .on_release(Message::SysTrayAction(SysTrayAction::LeftClick(
                    self.address.clone(),
                )))
                .on_right_release(Message::SysTrayAction(
                    SysTrayAction::RightClick(self.address.clone()),
                ))
                .into(),
                Icon::Raster(handle) => MouseArea::new(
                    Image::new(handle).width(ICON_SIZE).height(ICON_SIZE),
                )
                .into(),
            }
        } else {
            text("󰜺").size(16).into()
        }
    }
}

#[derive(Debug, Clone)]
pub enum SysTrayAction {
    LeftClick(String),
    RightClick(String),
}

pub struct SysTraySubscription {
    pub client: Arc<Client>,
}

impl subscription::Recipe for SysTraySubscription {
    type Output = Message;

    fn hash(&self, state: &mut subscription::Hasher) {
        std::ptr::hash(&self.client, state);
    }

    fn stream(
        self: Box<Self>,
        _input: subscription::EventStream,
    ) -> iced_runtime::futures::BoxStream<Self::Output> {
        let receiever: Receiver<Event> = self.client.subscribe();

        let broadcast_stream = BroadcastStream::new(receiever);

        let message_stream = broadcast_stream.filter_map(|result| match result {
            Ok(event) => Some(Message::SysTrayEvent(event)),
            Err(e) => {
                log::error!(" systray subscription error: {e}");
                None
            }
        });

        Box::pin(message_stream)
    }
}

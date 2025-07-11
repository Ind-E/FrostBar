use iced::{
    Element, Length,
    advanced::subscription,
    alignment::{Horizontal, Vertical},
    padding,
    widget::{Column, Container, Image, MouseArea, Svg, container, text},
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
    bar::{Message, MouseEvent},
    icon_cache::{Icon, IconCache},
};

const ICON_SIZE: u16 = 24;

#[derive(Debug)]
pub struct SysTrayModule {
    pub items: HashMap<String, SysTrayItem>,
    pub client: Option<Arc<Client>>,
    icon_cache: Arc<Mutex<IconCache>>,
}

impl SysTrayModule {
    pub async fn new(icon_cache: Arc<Mutex<IconCache>>) -> Self {
        let client_maybe = Client::new().await;
        let systray_client: Option<Client> = match client_maybe {
            Ok(client) => Some(client),
            Err(e) => {
                log::error!("{e}");
                None
            }
        };
        Self {
            items: HashMap::new(),
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
    pub id: container::Id,
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
            id: container::Id::unique(),
        }
    }

    fn to_widget<'a>(&self, icon_cache: &mut IconCache) -> Element<'a, Message> {
        if let Some(icon) = icon_cache.get_tray_icon(&self.inner).clone() {
            let image: Element<'a, Message> = match icon {
                Icon::Svg(handle) => {
                    Svg::new(handle).width(ICON_SIZE).height(ICON_SIZE).into()
                }
                Icon::Raster(handle) => {
                    Image::new(handle).width(ICON_SIZE).height(ICON_SIZE).into()
                }
            };
            Container::new(
                MouseArea::new(image)
                    .on_enter(Message::MouseEntered(MouseEvent::SysTrayItem(
                        self.address.clone(),
                    )))
                    .on_exit(Message::MouseExited(MouseEvent::SysTrayItem(
                        self.address.clone(),
                    )))
                    .on_release(Message::SysTrayAction(SysTrayAction::LeftClick(
                        self.address.clone(),
                    )))
                    .on_right_release(Message::SysTrayAction(
                        SysTrayAction::RightClick(self.address.clone()),
                    )),
            )
            .id(self.id.clone())
            .into()
        } else {
            text("󰜺").size(16).into()
        }
    }

    pub fn tooltip(&self) -> String {
        let mut tip = String::new();
        if let Some(menu) = &self.menu {
            for item in &menu.submenus {
                if item.visible
                    && let Some(label) = &item.label
                {
                    tip.push_str(&format!("{}\n", label));
                }
            }
        }
        tip
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

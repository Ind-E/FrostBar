use iced::{
    Element, Task,
    advanced::subscription,
    widget::{Image, MouseArea, Svg, svg, text},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use system_tray::{
    client::{Client, Event, UpdateEvent},
    item::StatusNotifierItem,
    menu::TrayMenu,
};
use tokio::sync::broadcast::Receiver;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::{
    Message,
    icon_cache::{Icon, IconCache},
};

const ICON_SIZE: u16 = 24;

pub struct SysTrayState {
    pub items: HashMap<String, Box<StatusNotifierItem>>,
}

#[derive(Debug, Clone)]
pub enum SysTrayInteraction {
    LeftClick(String),
    RightClick(String),
}

pub fn to_widget<'a>(
    id: &str,
    item: &StatusNotifierItem,
    icon_cache: &mut IconCache,
) -> Element<'a, Message> {
    let mut tray_icon = None;
    if let Some(icon) = icon_cache.get_tray_icon(item).clone() {
        tray_icon = Some(icon);
    }
    if let Some(icon) = tray_icon {
        match icon {
            Icon::Svg(handle) => {
                MouseArea::new(Svg::new(handle).width(ICON_SIZE).height(ICON_SIZE))
                    .on_release(Message::SysTrayInteraction(SysTrayInteraction::LeftClick(
                        id.to_string(),
                    )))
                    .on_right_release(Message::SysTrayInteraction(SysTrayInteraction::RightClick(
                        id.to_string(),
                    )))
                    .into()
            }
            Icon::Raster(handle) => {
                MouseArea::new(Image::new(handle).width(ICON_SIZE).height(ICON_SIZE)).into()
            }
        }
    } else {
        text("󰜺").size(16).into()
    }
}

impl SysTrayState {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    pub fn init(
        &mut self,
        items: Arc<Mutex<HashMap<String, (StatusNotifierItem, Option<TrayMenu>)>>>,
    ) {
        let items = items.lock().unwrap();
        items.iter().for_each(|(id, (item, _))| {
            self.items.insert(id.clone(), Box::new(item.clone()));
        });
    }

    pub fn on_event(&mut self, event: Event, icon_cache: &mut IconCache) {
        match event {
            Event::Add(id, item) => {
                self.items.insert(id, item);
            }
            Event::Update(id, update_event) => match update_event {
                UpdateEvent::Icon(icon_name) => {
                    if let Some(icon_name) = icon_name {
                        self.items.get_mut(&id).unwrap().icon_name = Some(icon_name);
                    }
                }
                UpdateEvent::AttentionIcon(icon_name) => {
                    println!("{:?}", icon_name);
                }
                UpdateEvent::OverlayIcon(icon_name) => {
                    println!("{:?}", icon_name);
                }
                // UpdateEvent::Status(status) => todo!(),
                // UpdateEvent::Title(_) => todo!(),
                // UpdateEvent::Tooltip(_) => todo!(),
                // UpdateEvent::Menu(tray_menu) => {}
                // UpdateEvent::MenuDiff(_) => todo!(),
                // UpdateEvent::MenuConnect(_) => todo!(),
                _ => {}
            },
            Event::Remove(id) => {
                self.items.remove(&id);
            }
        }
    }
}

pub fn create_client() -> Task<Message> {
    Task::perform(async { Client::new().await.unwrap() }, |client| {
        Message::SysTrayClientCreated(Arc::new(client))
    })
}

pub struct SysTraySubscription {
    pub client: Arc<Client>,
}

impl subscription::Recipe for SysTraySubscription {
    type Output = Message;

    fn hash(&self, state: &mut subscription::Hasher) {
        std::ptr::hash(&*self.client, state);
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
                eprintln!("{e}");
                None
            }
        });

        Box::pin(message_stream)
    }
}

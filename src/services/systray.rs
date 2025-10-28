use std::sync::{Arc, Mutex};

use iced::Subscription;
use rustc_hash::FxHashMap;
use system_tray::{
    client::{self, Client, UpdateEvent},
    data::{BaseMap, apply_menu_diffs},
    item::StatusNotifierItem,
    menu::TrayMenu,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::error;

use crate::{Message, module};

pub struct Systray {
    pub inner: FxHashMap<String, (StatusNotifierItem, Option<TrayMenu>)>,
}

impl Systray {
    pub fn new() -> Self {
        Self {
            inner: FxHashMap::default(),
        }
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::run(|| {
            let (yield_tx, yield_rx) = mpsc::channel(16);

            tokio::spawn(async move {
                let Ok(client) = Client::new().await else {
                    return;
                };
                if let Err(e) =
                    yield_tx.send(Event::InitialItems(client.items())).await
                {
                    error!("{e}");
                }

                let mut tray_rx = client.subscribe();
                while let Ok(event) = tray_rx.recv().await {
                    if let Err(e) = yield_tx.send(Event::Event(event)).await {
                        error!("{e}");
                    }
                }
            });

            ReceiverStream::new(yield_rx)
        })
        .map(|f| Message::Module(module::Message::Systray(f)))
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::InitialItems(mutex) => {
                for (k, v) in mutex.lock().unwrap().iter() {
                    self.inner.insert(k.to_string(), v.clone());
                }
            }
            Event::Event(event) => match event {
                client::Event::Add(name, status_notifier_item) => {
                    self.inner.insert(name, (*status_notifier_item, None));
                }
                client::Event::Update(name, update_event) => {
                    self.inner.get_mut(&name).map(|(sni, menu)| {
                        match update_event {
                            UpdateEvent::Icon {
                                icon_name,
                                icon_pixmap,
                            } => {
                                sni.icon_name = icon_name;
                                sni.icon_pixmap = icon_pixmap;
                            }
                            UpdateEvent::OverlayIcon(icon_name) => {
                                sni.overlay_icon_name = icon_name;
                            }
                            UpdateEvent::AttentionIcon(icon_name) => {
                                sni.attention_icon_name = icon_name;
                            }
                            UpdateEvent::Status(status) => {
                                sni.status = status;
                            }
                            UpdateEvent::Title(title) => {
                                sni.title = title;
                            }
                            UpdateEvent::Tooltip(tooltip) => {
                                sni.tool_tip = tooltip
                            }
                            UpdateEvent::Menu(tray_menu) => {
                                *menu = Some(tray_menu)
                            }
                            UpdateEvent::MenuDiff(diffs) => {
                                if let Some(menu) = menu {
                                    apply_menu_diffs(menu, &diffs);
                                }
                            }
                            UpdateEvent::MenuConnect(_) => {}
                        }
                    });
                }
                client::Event::Remove(name) => {
                    self.inner.remove(&name);
                }
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    InitialItems(Arc<Mutex<BaseMap>>),
    Event(client::Event),
}

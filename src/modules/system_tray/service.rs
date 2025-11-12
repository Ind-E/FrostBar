use crate::{
    Message,
    icon_cache::{Icon, IconCache},
    modules,
};
use iced::{Subscription, Task};
use rustc_hash::FxHashMap;
use std::sync::{Arc, Mutex};
use system_tray::{
    client::{self, ActivateRequest, Client, UpdateEvent},
    data::{BaseMap, apply_menu_diffs},
    item::StatusNotifierItem,
    menu::TrayMenu,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error};

pub struct TrayItem {
    pub address: String,
    pub id: String,
    pub title: Option<String>,
    pub tooltip: Option<TrayItemTooltip>,
    // icon_name: Option<String>,
    // icon_pixmaps: Option<Vec<IconPixmap>>,
    pub icon: Option<Icon>,
    pub overlay_icon: Option<Icon>,
    pub attention_icon: Option<Icon>,
    pub _dbus_menu: Option<String>,
    pub menu_only: bool,
}

pub struct TrayItemTooltip {
    pub title: String,
    pub description: String,
    pub icon: Option<Icon>,
}

pub struct SystemTrayService {
    pub items: FxHashMap<String, (TrayItem, Option<TrayMenu>)>,
    icon_cache: IconCache,
    client: Option<Arc<Client>>,
}

fn map_tooltip(
    icon_cache: &IconCache,
    tooltip: system_tray::item::Tooltip,
) -> TrayItemTooltip {
    TrayItemTooltip {
        title: tooltip.title,
        description: tooltip.description,
        icon: icon_cache
            .get_tray_icon(Some(tooltip.icon_name), Some(tooltip.icon_data)),
    }
}

#[profiling::all_functions]
impl SystemTrayService {
    pub fn new(icon_cache: IconCache) -> Self {
        Self {
            items: FxHashMap::default(),
            icon_cache,
            client: None,
        }
    }

    fn map_sni(&self, address: String, sni: StatusNotifierItem) -> TrayItem {
        TrayItem {
            id: sni.id,
            title: sni.title,
            tooltip: sni.tool_tip.map(|t| map_tooltip(&self.icon_cache, t)),
            icon: self
                .icon_cache
                .get_tray_icon(sni.icon_name, sni.icon_pixmap),
            overlay_icon: self
                .icon_cache
                .get_tray_icon(sni.overlay_icon_name, sni.overlay_icon_pixmap),
            attention_icon: self.icon_cache.get_tray_icon(
                sni.attention_icon_name,
                sni.attention_icon_pixmap,
            ),
            menu_only: sni.item_is_menu,
            _dbus_menu: sni.menu,
            address,
        }
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::run(|| {
            let (yield_tx, yield_rx) = mpsc::channel(16);

            tokio::spawn(async move {
                let Ok(client) = Client::new().await else {
                    error!(target: "system_tray", "failed to create client");
                    return;
                };
                let mut tray_rx = client.subscribe();
                if let Err(e) = yield_tx
                    .send(Event::InitialItems((
                        client.items(),
                        Arc::new(client),
                    )))
                    .await
                {
                    error!(target: "system_tray", "{e}");
                }

                while let Ok(event) = tray_rx.recv().await {
                    if let Err(e) = yield_tx.send(Event::Event(event)).await {
                        error!(target: "system_tray", "{e}");
                    }
                }
            });

            ReceiverStream::new(yield_rx)
        })
        .map(|f| Message::Module(modules::ModuleMsg::Systray(f)))
    }

    pub fn update(&mut self, event: Event) {
        match event {
            Event::InitialItems((mutex, client)) => {
                self.client = Some(client);
                for (address, (sni, menu)) in mutex.lock().unwrap().iter() {
                    self.items.insert(
                        address.clone(),
                        (
                            self.map_sni(address.clone(), sni.clone()),
                            menu.clone(),
                        ),
                    );
                }
            }
            Event::Event(event) => match event {
                client::Event::Add(address, status_notifier_item) => {
                    self.items.insert(
                        address.clone(),
                        (self.map_sni(address, *status_notifier_item), None),
                    );
                }
                client::Event::Update(name, update_event) => {
                    if let Some((sni, menu)) = self.items.get_mut(&name) {
                        match update_event {
                            UpdateEvent::Icon {
                                icon_name,
                                icon_pixmap,
                            } => {
                                sni.icon = self
                                    .icon_cache
                                    .get_tray_icon(icon_name, icon_pixmap);
                            }
                            UpdateEvent::OverlayIcon(icon_name) => {
                                sni.overlay_icon = self
                                    .icon_cache
                                    .get_tray_icon(icon_name, None);
                            }
                            UpdateEvent::AttentionIcon(icon_name) => {
                                sni.attention_icon = self
                                    .icon_cache
                                    .get_tray_icon(icon_name, None);
                            }
                            UpdateEvent::Status(_status) => {
                                // sni.status = status;
                            }
                            UpdateEvent::Title(title) => {
                                sni.title = title;
                            }
                            UpdateEvent::Tooltip(tooltip) => {
                                sni.tooltip = tooltip
                                    .map(|t| map_tooltip(&self.icon_cache, t));
                            }
                            UpdateEvent::Menu(tray_menu) => {
                                *menu = Some(tray_menu);
                            }
                            UpdateEvent::MenuDiff(diffs) => {
                                if let Some(menu) = menu {
                                    apply_menu_diffs(menu, &diffs);
                                }
                            }
                            UpdateEvent::MenuConnect(_) => {}
                        }
                    }
                }
                client::Event::Remove(name) => {
                    self.items.remove(&name);
                }
            },
        }
    }

    pub fn activate_menu(&mut self, address: String) -> Option<Task<Message>> {
        if let Some(client) = self.client.clone() {
            debug!(target: "system_tray", "activating {address}");
            Some(Task::perform(
                async move {
                    if let Err(e) = client
                        .activate(ActivateRequest::Default {
                            address,
                            x: 0,
                            y: 0,
                        })
                        .await
                    {
                        error!(target: "system_tray", "{e}");
                    }
                },
                |_| Message::NoOp,
            ))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    InitialItems((Arc<Mutex<BaseMap>>, Arc<Client>)),
    Event(client::Event),
}

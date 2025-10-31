use std::sync::{Arc, Mutex};

use iced::{Subscription, widget::image};
use rustc_hash::FxHashMap;
use system_tray::{
    client::{self, Client, UpdateEvent},
    data::{BaseMap, apply_menu_diffs},
    item::{IconPixmap, StatusNotifierItem},
    menu::TrayMenu,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::error;

use crate::{
    Message,
    icon_cache::{Icon, IconCache},
    module,
};

pub struct TrayItem {
    pub id: String,
    pub title: Option<String>,
    icon_name: Option<String>,
    icon_pixmaps: Option<Vec<IconPixmap>>,
    pub icon: Option<Icon>,
    pub dbus_menu: Option<String>,
    // overlay_icon_name: Option<String>,
    // overlay_icon_pixmap: Option<Vec<Pixmap>>,
    // attention_icon_name: Option<String>,
    // attention_icon_pixmap: Option<Vec<Pixmap>>,
}

pub fn handle_from_pixmaps(
    pixmaps: Vec<IconPixmap>,
    size: i32,
) -> Option<Icon> {
    pixmaps
        .into_iter()
        .max_by(
            |IconPixmap {
                 width: w1,
                 height: h1,
                 ..
             },
             IconPixmap {
                 width: w2,
                 height: h2,
                 ..
             }| {
                (w1 * h1).cmp(&(w2 * h2))
                // take smallest one bigger than requested size, otherwise take biggest
                // let a = size * size;
                // let a1 = w1 * h1;
                // let a2 = w2 * h2;
                // match (a1 >= a, a2 >= a) {
                //     (true, true) => a2.cmp(&a1),
                //     (true, false) => std::cmp::Ordering::Greater,
                //     (false, true) => std::cmp::Ordering::Less,
                //     (false, false) => a1.cmp(&a2),
                // }
            },
        )
        .and_then(|IconPixmap { pixels, .. }| {
            let handle = image::Handle::from_bytes(pixels);
            Some(Icon::Raster(handle))
        })
}

pub struct Systray {
    pub inner: FxHashMap<String, (TrayItem, Option<TrayMenu>)>,
    icon_cache: IconCache,
}

impl Systray {
    pub fn new(icon_cache: IconCache) -> Self {
        Self {
            inner: FxHashMap::default(),
            icon_cache,
        }
    }

    fn map_sni(&self, sni: StatusNotifierItem) -> TrayItem {
        let icon = if let Some(icon_name) = &sni.icon_name
            && let Some(icon) = self.icon_cache.get_icon(icon_name)
        {
            Some(icon)
        } else if let Some(icon_pixmaps) = &sni.icon_pixmap {
            handle_from_pixmaps(icon_pixmaps.clone(), 32)
        } else {
            None
        };
        TrayItem {
            id: sni.id,
            title: sni.title,
            icon,
            icon_name: sni.icon_name,
            icon_pixmaps: sni.icon_pixmap,
            dbus_menu: sni.menu,
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
                for (k, (sni, menu)) in mutex.lock().unwrap().iter() {
                    self.inner.insert(
                        k.to_string(),
                        (self.map_sni(sni.clone()), menu.clone()),
                    );
                }
            }
            Event::Event(event) => match event {
                client::Event::Add(name, status_notifier_item) => {
                    self.inner.insert(
                        name,
                        (self.map_sni(*status_notifier_item), None),
                    );
                }
                client::Event::Update(name, update_event) => {
                    self.inner.get_mut(&name).map(|(sni, menu)| {
                        match update_event {
                            UpdateEvent::Icon {
                                icon_name,
                                icon_pixmap,
                            } => {
                                sni.icon_name = icon_name;
                                sni.icon_pixmaps = icon_pixmap;
                            }
                            UpdateEvent::OverlayIcon(icon_name) => {
                                // sni.overlay_icon_name = icon_name;
                            }
                            UpdateEvent::AttentionIcon(icon_name) => {
                                // sni.attention_icon_name = icon_name;
                            }
                            UpdateEvent::Status(status) => {
                                // sni.status = status;
                            }
                            UpdateEvent::Title(title) => {
                                sni.title = title;
                            }
                            UpdateEvent::Tooltip(tooltip) => {
                                // sni.tool_tip = tooltip
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

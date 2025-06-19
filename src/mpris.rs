use futures_util::stream::{Stream, StreamExt, select_all};
use iced::{
    Element,
    advanced::subscription,
    widget::{Image, MouseArea, image, text},
};
use std::{collections::HashMap, hash::Hash, pin::Pin};
use zbus::{Connection, Proxy, zvariant::OwnedValue};

use crate::{Message, icon_cache::MprisArtCache, mpris_player::PlayerProxy};

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";

type EventStream = Pin<Box<dyn Stream<Item = Result<MprisEvent, zbus::Error>> + Send>>;

#[derive(Clone, Debug)]
pub enum MprisEvent {
    PlayerAppeared {
        name: String,
        status: String,
        metadata: HashMap<String, OwnedValue>,
    },
    PlayerVanished {
        name: String,
    },
    PlaybackStatusChanged {
        player_name: String,
        status: String,
    },
    MetadataChanged {
        player_name: String,
        metadata: HashMap<String, OwnedValue>,
    },
}

#[derive(Clone, Debug)]
pub struct MprisPlayer {
    pub status: String,
    pub artists: Option<String>,
    pub title: Option<String>,
    pub art: Option<image::Handle>,
}

impl MprisPlayer {
    pub fn update_metadata(
        &mut self,
        metadata: &HashMap<String, OwnedValue>,
        art_cache: &mut MprisArtCache,
    ) {
        if let Some(val) = metadata.get("xesam:title") {
            self.title = Some(val.to_string());
        }
        if let Some(val) = metadata.get("xesam:artist") {
            self.artists = Some(val.to_string());
        }
        if let Some(val) = metadata.get("mpris:artUrl") {
            self.art = art_cache
                .get_art(
                    &val.to_string()
                        .strip_prefix("\"")
                        .unwrap()
                        .strip_suffix("\"")
                        .unwrap(),
                )
                .clone();
        }
    }

    pub fn new(status: String) -> Self {
        Self {
            status,
            artists: None,
            title: None,
            art: None,
        }
    }

    pub fn to_widget<'a>(&self) -> Element<'a, Message> {
        if let Some(art) = &self.art {
            MouseArea::new(Image::new(art)).into()
        } else {
            text("X").into()
        }
    }
}

pub struct MprisListener;

impl subscription::Recipe for MprisListener {
    type Output = MprisEvent;

    fn hash(&self, state: &mut subscription::Hasher) {
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: subscription::EventStream,
    ) -> iced_runtime::futures::BoxStream<Self::Output> {
        Box::pin(async_stream::stream! {

            let connection = match Connection::session().await {
                Ok(c) => c,
                Err(_) => return,
            };

            let dbus_proxy = Proxy::new(
                &connection,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            ).await.unwrap();

            let mut player_streams = HashMap::new();

        if let Ok(names) = dbus_proxy.call_method("ListNames", &()).await {
            if let Ok(names) = names.body().deserialize::<Vec<String>>() {
            for name in names {
                if name.starts_with(MPRIS_PREFIX) {
                    let event = get_initial_player_state(&connection, &name).await;
                    yield event;

                    let _ = add_player_listener(&connection, &mut player_streams, name).await;
                }
            }
            }
        }

            let mut name_owner_stream = dbus_proxy.receive_signal("NameOwnerChanged").await.unwrap();

            loop {
                tokio::select! {
                    biased;

                    Some(signal) = name_owner_stream.next() => {
                        if let Ok((name, old, new)) = signal.body().deserialize::<(String, String, String)>() {
                            if name.starts_with(MPRIS_PREFIX) {
                                if !new.is_empty() {
                                    let event = get_initial_player_state(&connection, &name).await;
                                    yield event;

                                    let _ = add_player_listener(&connection, &mut player_streams, name.clone()).await;
                                } else if !old.is_empty() {
                                    player_streams.remove(&name);
                                    let event = MprisEvent::PlayerVanished { name };
                                    yield event;
                                }
                            }
                        }
                    },

                    Some(event_result) = poll_player_streams(&mut player_streams) => {
                        if let Ok(event) = event_result {
                            yield event;
                        }
                    }
                }
            }
        })
    }
}

async fn get_initial_player_state(connection: &Connection, name: &str) -> MprisEvent {
    let proxy = PlayerProxy::new(connection, name).await.unwrap();
    let status = proxy.playback_status().await.unwrap();
    let metadata = proxy.metadata().await.unwrap();
    MprisEvent::PlayerAppeared {
        name: name.to_string(),
        status,
        metadata,
    }
}

async fn add_player_listener(
    connection: &Connection,
    player_streams: &mut HashMap<String, EventStream>,
    name: String,
) -> Result<(), zbus::Error> {
    let player_proxy = PlayerProxy::new(&connection, name.clone()).await?;
    let mut streams: Vec<EventStream> = vec![];

    let player_name = name.clone();

    let playback_stream = player_proxy
        .receive_playback_status_changed()
        .await
        .map(move |p| {
            let player_name = player_name.clone();
            async move {
                let status = p.get().await?;
                Ok(MprisEvent::PlaybackStatusChanged {
                    player_name,
                    status,
                })
            }
        })
        .buffer_unordered(1)
        .boxed();
    streams.push(playback_stream);

    let player_name = name.clone();

    let metadata_stream = player_proxy
        .receive_metadata_changed()
        .await
        .map(move |p| {
            let player_name = player_name.clone();
            async move {
                let metadata = p.get().await?;
                Ok(MprisEvent::MetadataChanged {
                    player_name,
                    metadata,
                })
            }
        })
        .buffer_unordered(1)
        .boxed();
    streams.push(metadata_stream);
    let merged = select_all(streams).boxed();

    player_streams.insert(name, merged);
    Ok(())
}

async fn poll_player_streams(
    player_streams: &mut HashMap<String, EventStream>,
) -> Option<Result<MprisEvent, zbus::Error>> {
    if player_streams.is_empty() {
        futures_util::future::pending::<()>().await;
        return None;
    }

    let mut all = select_all(player_streams.values_mut());

    all.next().await
}

use iced::{
    Color, Subscription,
    futures::{StreamExt, stream::select_all},
    widget::image,
};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_stream::{StreamMap, wrappers::UnboundedReceiverStream};
use zbus::{Connection, Proxy, zvariant::OwnedValue};

use tracing::error;

use crate::{
    Message, dbus_proxy::PlayerProxy, icon_cache::MprisArtCache,
    services::Service, utils::BoxStream,
};

pub struct MprisService {
    pub players: HashMap<String, MprisPlayer>,
    art_cache: MprisArtCache,
}

#[profiling::all_functions]
impl Service for MprisService {
    fn subscription() -> iced::Subscription<Message> {
        Subscription::run(|| {
        let (yield_tx, yield_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let connection = match Connection::session().await {
                Ok(c) => c,
                Err(e) => {
                    error!("mpris stream error: {e}");
                    return;
                }
            };

            let dbus_proxy = Proxy::new(
                &connection,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            ).await.unwrap();

            let mut player_streams = StreamMap::new();

            if let Ok(names) = dbus_proxy.call_method("ListNames", &()).await &&
                let Ok(names) = names.body().deserialize::<Vec<String>>() {
                for name in names {
                    let name1 = name.clone();
                    if name.starts_with(MPRIS_PREFIX) {
                        if let Err(e) = yield_tx.send(get_initial_player_state(&connection, &name).await) {
                            error!("{e}");
                        }

                        if let Ok(stream) = create_player_stream(&connection, name).await {
                            player_streams.insert(name1, stream.fuse());
                        }
                    }
                }
            }

            let mut name_owner_stream = dbus_proxy.receive_signal("NameOwnerChanged").await.unwrap().fuse();

            loop {
                tokio::select! {
                    signal = name_owner_stream.next() => {
                        if let Some(signal) = signal
                            && let Ok((name, old, new)) = signal.body().deserialize::<(String, String, String)>()
                            && name.starts_with(MPRIS_PREFIX)
                        {
                            if !new.is_empty() && old.is_empty() {
                                if let Err(e) = yield_tx.send(get_initial_player_state(&connection, &name).await) {
                                    error!("{e}");
                                }

                                let name1 = name.clone();
                                if let Ok(stream) = create_player_stream(&connection, name).await {
                                    player_streams.insert(name1, stream.fuse());
                                }
                            } else if new.is_empty() && !old.is_empty()
                                && let Err(e) = yield_tx.send( MprisEvent::PlayerVanished { name }) {
                                    error!("{e}");
                                }
                        }
                    },

                    event_result = player_streams.next(), if !player_streams.is_empty() => {
                        if let Some((pname, Ok(event))) = event_result {
                            if let MprisEvent::PlayerVanished {ref name} = event
                                &&  *name == pname {
                                    player_streams.remove(&pname);
                                }
                            if let Err(e) = yield_tx.send(event) {
                                error!("{e}");
                            }
                        }
                    }

                }
            }

        });

        UnboundedReceiverStream::new(yield_rx)


        }).map(Message::MprisEvent)
    }

    type Event = MprisEvent;
    fn handle_event(&mut self, event: Self::Event) -> iced::Task<Message> {
        match event {
            MprisEvent::PlayerAppeared {
                name,
                status,
                metadata,
            } => {
                let mut player = MprisPlayer::new(name.clone(), status);
                let task =
                    player.update_metadata(&metadata, &mut self.art_cache);
                self.players.insert(name, player);
                task
            }
            MprisEvent::PlayerVanished { name } => {
                self.players.remove(&name);

                let players_with_colors = self
                    .players
                    .iter()
                    .filter(|(_, p)| {
                        p.colors.is_some()
                            && p.status == "Playing"
                            && p.name != name
                    })
                    .collect::<Vec<_>>();

                if players_with_colors.is_empty() {
                    return iced::Task::perform(
                        async move { None },
                        Message::CavaColorUpdate,
                    );
                }
                return iced::Task::none();
            }
            MprisEvent::PlaybackStatusChanged {
                player_name,
                status,
            } => {
                if let Some(player) = self.players.get_mut(&player_name) {
                    if status == "Playing" {
                        player.status = status;
                        let colors = player.colors.clone();
                        return iced::Task::perform(
                            async move { colors },
                            Message::CavaColorUpdate,
                        );
                    }
                    player.status = status;
                    let name = player.name.clone();
                    let players_with_colors = self
                        .players
                        .iter()
                        .filter(|(_, p)| {
                            p.colors.is_some()
                                && p.status == "Playing"
                                && p.name != name
                        })
                        .collect::<Vec<_>>();
                    if let Some((_, player)) = players_with_colors.first() {
                        let colors = player.colors.clone();
                        return iced::Task::perform(
                            async move { colors },
                            Message::CavaColorUpdate,
                        );
                    }
                    return iced::Task::perform(
                        async move { None },
                        Message::CavaColorUpdate,
                    );
                }
                iced::Task::none()
            }
            MprisEvent::MetadataChanged {
                player_name,
                metadata,
            } => {
                if let Some(player) = self.players.get_mut(&player_name) {
                    player.update_metadata(&metadata, &mut self.art_cache)
                } else {
                    iced::Task::none()
                }
            }
        }
    }
}

impl MprisService {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            art_cache: MprisArtCache::new(),
        }
    }
}

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";

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
    pub name: String,
    pub status: String,
    pub artists: Option<String>,
    pub title: Option<String>,
    pub art: Option<image::Handle>,
    pub colors: Option<Vec<Color>>,
}

#[profiling::all_functions]
impl MprisPlayer {
    pub fn update_metadata(
        &mut self,
        metadata: &HashMap<String, OwnedValue>,
        art_cache: &mut MprisArtCache,
    ) -> iced::Task<Message> {
        if let Some(val) = metadata.get("xesam:title") {
            self.title = Some(val.to_string());
        }
        if let Some(val) = metadata.get("xesam:artist") {
            self.artists = Some(val.to_string());
        }

        if let Some(val) = metadata.get("mpris:artUrl") {
            let art_url = val.to_string().trim_matches('"').to_string();
            if let Some((handle, colors)) = art_cache.get_art(&art_url) {
                self.art = Some(handle.clone());
                self.colors.clone_from(colors);
                if self.status == "Playing" {
                    let captured_colors = colors.clone();
                    return iced::Task::perform(
                        async move { captured_colors },
                        Message::CavaColorUpdate,
                    );
                }
            } else {
                self.art = None;
                self.colors = None;
            }
            return iced::Task::none();
        }

        self.art = None;
        self.colors = None;
        iced::Task::perform(async { None }, Message::CavaColorUpdate)
    }

    pub fn new(name: String, status: String) -> Self {
        Self {
            name,
            status,
            artists: None,
            title: None,
            art: None,
            colors: None,
        }
    }
}

#[tracing::instrument]
async fn get_initial_player_state(
    connection: &Connection,
    name: &str,
) -> MprisEvent {
    let proxy = PlayerProxy::new(connection, name).await.unwrap();
    let status = proxy.playback_status().await.unwrap();
    let metadata = proxy.metadata().await.unwrap();
    MprisEvent::PlayerAppeared {
        name: name.to_string(),
        status,
        metadata,
    }
}

#[tracing::instrument]
async fn create_player_stream(
    connection: &Connection,
    name: String,
) -> Result<BoxStream<Result<MprisEvent, zbus::Error>>, zbus::Error> {
    let player_proxy = PlayerProxy::new(connection, name.clone()).await?;
    let mut streams: Vec<BoxStream<Result<MprisEvent, zbus::Error>>> = vec![];
    {
        let name = name.clone();
        let playback_stream = player_proxy
            .receive_playback_status_changed()
            .await
            .map(move |p| {
                let player_name = name.clone();
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
    }

    {
        let name = name.clone();
        let metadata_stream = player_proxy
            .receive_metadata_changed()
            .await
            .map(move |p| {
                let player_name = name.clone();
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
    }

    Ok(select_all(streams).boxed())
}

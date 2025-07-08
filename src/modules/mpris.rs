use futures_util::stream::{Stream, StreamExt, select_all};
use iced::{
    Border, Color, Element, Length,
    advanced::subscription,
    border::Radius,
    mouse::Interaction,
    widget::{Column, Container, Image, MouseArea, container, image, text},
};
use std::{collections::HashMap, hash::Hash, pin::Pin};
use zbus::{Connection, Proxy, zvariant::OwnedValue};

use crate::{
    BAR_WIDTH,
    bar::{Message, MouseEvent},
    dbus_proxy::PlayerProxy,
    icon_cache::MprisArtCache,
};

pub struct MprisState {
    pub players: HashMap<String, MprisPlayer>,
    art_cache: MprisArtCache,
}

impl MprisState {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            art_cache: MprisArtCache::new(),
        }
    }

    pub fn on_event(&mut self, event: MprisEvent) -> iced::Task<Message> {
        match event {
            MprisEvent::PlayerAppeared {
                name,
                status,
                metadata,
            } => {
                let mut player = MprisPlayer::new(name.clone(), status);
                player.update_metadata(&metadata, &mut self.art_cache);
                self.players.insert(name, player);
            }
            MprisEvent::PlayerVanished { name } => {
                self.players.remove(&name);
            }
            MprisEvent::PlaybackStatusChanged {
                player_name,
                status,
            } => {
                if let Some(player) = self.players.get_mut(&player_name) {
                    player.status = status;
                }
            }
            MprisEvent::MetadataChanged {
                player_name,
                metadata,
            } => {
                if let Some(player) = self.players.get_mut(&player_name) {
                    player.update_metadata(&metadata, &mut self.art_cache);
                }
            }
        };
        iced::Task::none()
    }

    pub fn to_widget<'a>(&self) -> Element<'a, Message> {
        self.players
            .values()
            .fold(Column::new().spacing(5).padding(5), |col, player| {
                col.push(player.to_widget())
            })
            .into()
    }
}

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";

type EventStream =
    Pin<Box<dyn Stream<Item = Result<MprisEvent, zbus::Error>> + Send>>;

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
    name: String,
    status: String,
    artists: Option<String>,
    title: Option<String>,
    art: Option<image::Handle>,
    pub id: container::Id,
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
        } else {
            self.art = None
        }
    }

    pub fn new(name: String, status: String) -> Self {
        Self {
            name,
            status,
            artists: None,
            title: None,
            art: None,
            id: container::Id::unique(),
        }
    }

    pub fn tooltip(&self) -> String {
        let raw_artists =
            self.artists.clone().unwrap_or_else(|| "[]".to_string());
        let raw_title = self.title.clone().unwrap_or_else(|| "\"\"".to_string());

        let artists = raw_artists
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().trim_matches('"'))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");

        let title = raw_title.trim().trim_matches('"');

        format!("{} - {}", artists, title)
    }

    pub fn to_widget<'a>(&self) -> Element<'a, Message> {
        let content: Element<'a, Message> = if let Some(art) = &self.art {
            Container::new(Image::new(art)).into()
        } else {
            Container::new(
                text("󰝚")
                    .size(18)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center(),
            )
            .padding(5)
            .width(BAR_WIDTH as u16 - 16)
            .height(BAR_WIDTH as u16 - 16)
            .style(|_| container::Style {
                border: Border {
                    color: Color::WHITE,
                    width: 1.0,
                    radius: Radius::new(1),
                },
                ..Default::default()
            })
            .into()
        };

        Container::new(
            MouseArea::new(content)
                .on_enter(Message::MouseEntered(MouseEvent::MprisPlayer(
                    self.name.clone(),
                )))
                .on_exit(Message::MouseExited(MouseEvent::MprisPlayer(
                    self.name.clone(),
                )))
                .on_release(Message::PlayPause(self.name.clone()))
                .on_right_release(Message::NextSong(self.name.clone()))
                .interaction(Interaction::Pointer),
        )
        .id(self.id.clone())
        .into()
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
                Err(e) => {
                    log::error!("mpris stream error: {e}");
                    return;
                }
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

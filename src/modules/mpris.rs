use iced::{
    Border, Color, Element, Length,
    advanced::subscription,
    border::Radius,
    futures::{self, FutureExt, Stream, StreamExt, stream::select_all},
    mouse::Interaction,
    widget::{Column, Container, Image, MouseArea, Text, container, image, text},
};
use std::{collections::HashMap, hash::Hash, pin::Pin};
use zbus::{Connection, Proxy, zvariant::OwnedValue};

use tracing::error;

use crate::{
    Message,
    config::{Cava, Layout},
    dbus_proxy::PlayerProxy,
    icon_cache::MprisArtCache,
    style::styled_tooltip,
};

pub struct MprisModule {
    pub players: HashMap<String, MprisPlayer>,
    art_cache: MprisArtCache,
}

impl MprisModule {
    pub fn new(cava: Cava) -> Self {
        Self {
            players: HashMap::new(),
            art_cache: MprisArtCache::new(cava),
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
                let task = player.update_metadata(&metadata, &mut self.art_cache);
                self.players.insert(name, player);
                task
            }
            MprisEvent::PlayerVanished { name } => {
                self.players.remove(&name);
                iced::Task::none()
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
                    let players_w_colors = self
                        .players
                        .iter()
                        .filter(|(_, p)| {
                            p.colors.is_some() && p.status == "Playing" && p.name != name
                        })
                        .collect::<Vec<_>>();
                    if let Some((_, player)) = players_w_colors.get(0) {
                        let colors = player.colors.clone();
                        return iced::Task::perform(
                            async move { colors },
                            Message::CavaColorUpdate,
                        );
                    } else {
                        return iced::Task::perform(
                            async move { None },
                            Message::CavaColorUpdate,
                        );
                    }
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

    pub fn to_widget<'a>(&self, layout: &Layout) -> Element<'a, Message> {
        self.players
            .values()
            .fold(Column::new().spacing(5).padding(5), |col, player| {
                col.push(player.to_widget(&layout))
            })
            .into()
    }
}

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
    name: String,
    status: String,
    artists: Option<String>,
    title: Option<String>,
    art: Option<image::Handle>,
    colors: Option<Vec<Color>>,
    pub id: container::Id,
}

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

        let task = if let Some(val) = metadata.get("mpris:artUrl") {
            let art_url = val.to_string().trim_matches('"').to_string();
            let (handle, colors) = art_cache.get_art(&art_url);

            self.art = handle.clone();
            self.colors = colors.clone();

            if self.status == "Playing" {
                let captured_colors = colors.clone();

                iced::Task::perform(
                    async move { captured_colors },
                    Message::CavaColorUpdate,
                )
            } else {
                iced::Task::none()
            }
        } else {
            self.art = None;
            self.colors = None;
            iced::Task::perform(async { None }, Message::CavaColorUpdate)
        };
        task
    }

    pub fn new(name: String, status: String) -> Self {
        Self {
            name,
            status,
            artists: None,
            title: None,
            art: None,
            colors: None,
            id: container::Id::unique(),
        }
    }

    pub fn to_widget<'a>(&self, layout: &Layout) -> Element<'a, Message> {
        let content: Element<'a, Message> = if let Some(art) = &self.art {
            Container::new(Image::new(art)).into()
        } else {
            Container::new(
                text("󰝚")
                    .size(20)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center(),
            )
            .padding(5)
            .width(layout.width - layout.gaps as u32 * 4)
            .height(layout.width - layout.gaps as u32 * 4)
            .center_x(Length::Fill)
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

        let raw_artists = self.artists.clone().unwrap_or_else(|| "[]".to_string());
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

        let tooltip = Text::new(format!("{} - {}", artists, title));

        let content = Container::new(
            MouseArea::new(content)
                .on_release(Message::PlayPause(self.name.clone()))
                .on_right_release(Message::NextSong(self.name.clone()))
                .on_middle_release(Message::StopPlayer(self.name.clone()))
                .interaction(Interaction::Pointer),
        )
        .id(self.id.clone());

        styled_tooltip(content, tooltip)
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
    ) -> Pin<Box<dyn Stream<Item = Self::Output> + Send>> {
        Box::pin(async_stream::stream! {

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

            let mut player_streams = select_all(Vec::new());
                // HashMap::new();

        if let Ok(names) = dbus_proxy.call_method("ListNames", &()).await &&
            let Ok(names) = names.body().deserialize::<Vec<String>>() {
            for name in names {
                if name.starts_with(MPRIS_PREFIX) {
                    yield get_initial_player_state(&connection, &name).await;

                    if let Ok(stream) = create_player_stream(&connection, name).await {
                        player_streams.push(stream);
                    }
                }
            }
        }

        let mut name_owner_stream = dbus_proxy.receive_signal("NameOwnerChanged").await.unwrap();

            loop {
                futures::select! {
                    signal = name_owner_stream.next().fuse() => {
                        if let Some(signal) = signal && let Ok((name, old, new)) = signal.body().deserialize::<(String, String, String)>() {
                            if name.starts_with(MPRIS_PREFIX) {


                                if !new.is_empty() && old.is_empty() {
                                    yield get_initial_player_state(&connection, &name).await;
                                    if let Ok(stream) = create_player_stream(&connection, name).await {
                                        player_streams.push(stream);
                                    }
                                } else if new.is_empty() && !old.is_empty() {
                                    yield MprisEvent::PlayerVanished { name };
                                }
                            }
                        }
                    },

                    event_result = player_streams.next().fuse() => {
                        if let Some(Ok(event)) = event_result {
                            yield event;
                        }
                    }

                    complete => break,
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
async fn create_player_stream(
    connection: &Connection,
    name: String,
) -> Result<EventStream, zbus::Error> {
    let player_proxy = PlayerProxy::new(&connection, name.clone()).await?;
    let mut streams: Vec<EventStream> = vec![];
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

use std::collections::HashMap;

use iced::{
    Color, Subscription,
    advanced::graphics::image::image_rs,
    futures::{StreamExt, stream::select_all},
    widget::image,
};
use rustc_hash::FxHashMap;
use tokio::sync::mpsc;
use tokio_stream::{StreamMap, wrappers::UnboundedReceiverStream};
use zbus::{Connection, Proxy, zvariant::OwnedValue};

use base64::Engine;

use tracing::{debug, error};

use crate::{
    Message, ModuleMessage, dbus_proxy::PlayerProxy, services::Service,
    utils::BoxStream,
};

pub struct MprisService {
    pub players: FxHashMap<String, MprisPlayer>,
}

#[profiling::all_functions]
impl Service for MprisService {
    fn subscription() -> iced::Subscription<Message> {
        Subscription::run(|| {
        let (yield_tx, yield_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            profiling::register_thread!("mpris watcher");
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


        }).map(|f| Message::Msg(ModuleMessage::Mpris(f)))
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
                let task = player.update_metadata(&metadata);
                self.players.insert(name, player);
                task
            }
            MprisEvent::PlayerVanished { name } => {
                debug!("player vanished: {name}");
                self.players.remove(&name);

                return iced::Task::none();
            }
            MprisEvent::PlaybackStatusChanged {
                player_name,
                status,
            } => {
                debug!("{player_name} status changed: {status}");
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
                    let players_with_colors = self
                        .players
                        .iter()
                        .filter(|(_, p)| {
                            p.colors.is_some() && p.status == "Playing"
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
                    player.update_metadata(&metadata)
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
            players: FxHashMap::default(),
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
    ) -> iced::Task<Message> {
        if let Some(val) = metadata.get("xesam:title") {
            self.title = Some(val.to_string());
        }
        if let Some(val) = metadata.get("xesam:artist") {
            self.artists = Some(val.to_string());
        }

        if let Some(val) = metadata.get("mpris:artUrl") {
            let art_url = val.to_string().trim_matches('"').to_string();
            match self.get_art(art_url) {
                PlayerArt::Async(task) => {
                    return task;
                }
                PlayerArt::Sync(art) => {
                    if let Some((handle, colors)) = art {
                        self.art = Some(handle);
                        self.colors.clone_from(&colors);
                        if self.status == "Playing" {
                            let captured_colors = colors;
                            return iced::Task::perform(
                                async move { captured_colors },
                                Message::CavaColorUpdate,
                            );
                        }
                    }
                }
                PlayerArt::None => {
                    self.art = None;
                    self.colors = None;
                    return iced::Task::none();
                }
            }
        }

        self.art = None;
        self.colors = None;
        iced::Task::perform(async { None }, Message::CavaColorUpdate)
    }

    pub fn get_art(&self, art_url: String) -> PlayerArt {
        if let Some(url) = art_url.strip_prefix("data:image/jpeg;base64,") {
            let image_bytes =
                match base64::engine::general_purpose::STANDARD.decode(url) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        error!("base64 decode error: {e}");
                        return PlayerArt::None;
                    }
                };
            let gradient = image_rs::load_from_memory(&image_bytes)
                .ok()
                .and_then(|img| extract_gradient(&img.to_rgb8(), 12));
            let handle = image::Handle::from_bytes(image_bytes);
            PlayerArt::Sync(Some((handle, gradient)))
        } else if let Some(url) = art_url.strip_prefix("file://") {
            let handle = image::Handle::from_path(url);
            let gradient = image_rs::open(url)
                .ok()
                .and_then(|img| extract_gradient(&img.to_rgb8(), 12));
            PlayerArt::Sync(Some((handle, gradient)))
        } else if art_url.starts_with("https://")
            || art_url.starts_with("http://")
        {
            let name = self.name.clone();
            let task = iced::Task::perform(
                async move {
                    let response = match reqwest::get(&art_url).await {
                        Ok(res) => res,
                        Err(e) => {
                            error!("Failed to fetch album art: {e}");
                            return None;
                        }
                    };
                    let image_bytes = match response.bytes().await {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            error!(
                                "Failed to get bytes of album art from {art_url}: {e}"
                            );
                            return None;
                        }
                    };

                    let gradient = image_rs::load_from_memory(&image_bytes)
                        .ok()
                        .and_then(|img| extract_gradient(&img.to_rgb8(), 12));
                    let handle = image::Handle::from_bytes(image_bytes);
                    Some((handle, gradient))
                },
                |art| Message::PlayerArtUpdate(name, art),
            );

            return PlayerArt::Async(task);
        } else {
            PlayerArt::None
        }
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

enum PlayerArt {
    Async(iced::Task<Message>),
    Sync(Option<(image::Handle, Option<Vec<Color>>)>),
    None,
}

#[profiling::function]
fn generate_gradient(
    palette: Vec<color_thief::Color>,
    steps: usize,
) -> Option<Vec<Color>> {
    if palette.is_empty() {
        return None;
    }

    let iced_palette: Vec<Color> = palette
        .into_iter()
        .map(|c| Color::from_rgb8(c.r, c.g, c.b))
        .collect();

    if iced_palette.len() == 1 {
        return Some(vec![iced_palette[0]; steps]);
    }

    let mut gradient = Vec::with_capacity(steps);
    let segments = (iced_palette.len() - 1) as f32;

    for i in 0..steps {
        let progress = if steps == 1 {
            0.0
        } else {
            i as f32 / (steps - 1) as f32
        };
        let position = progress * segments;

        let start_index = position.floor() as usize;
        let end_index = (start_index + 1).min(iced_palette.len() - 1);

        let factor = position.fract();

        let start_color = iced_palette[start_index];
        let end_color = iced_palette[end_index];

        gradient.push(lerp_color(start_color, end_color, factor));
    }

    Some(gradient)
}

fn lerp_color(c1: Color, c2: Color, factor: f32) -> Color {
    let r = c1.r * (1.0 - factor) + c2.r * factor;
    let g = c1.g * (1.0 - factor) + c2.g * factor;
    let b = c1.b * (1.0 - factor) + c2.b * factor;
    Color::from_rgba(r, g, b, 1.0)
}

#[profiling::function]
fn extract_gradient(
    buffer: &image_rs::ImageBuffer<image_rs::Rgb<u8>, Vec<u8>>,
    bars: usize,
) -> Option<Vec<Color>> {
    match color_thief::get_palette(
        buffer.as_raw(),
        color_thief::ColorFormat::Rgb,
        10,
        3,
    ) {
        Ok(palette) => generate_gradient(palette, bars * 2),
        Err(_) => None,
    }
}

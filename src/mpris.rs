use iced::advanced::{image, subscription};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use tokio::time::{self, Duration};
use url::Url;
use zbus::{Connection, Proxy, zvariant::Value};

#[derive(Clone, Debug)]
pub struct PlayerState {
    pub unique_name: String,
    pub title: String,
    pub artist: String,
    pub art_handle: Option<image::Handle>,
}

#[derive(Clone, Debug)]
pub enum MprisUpdate {
    PlayerChanged(PlayerState),
    PlayerVanished(String),
}

pub struct MprisListener;


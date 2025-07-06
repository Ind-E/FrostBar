use iced::{
    Element, Task,
    advanced::subscription,
    widget::{Image, MouseArea, Svg, text},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast::Receiver;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::{
    bar::Message,
    icon_cache::{Icon, IconCache},
};

const ICON_SIZE: u16 = 24;


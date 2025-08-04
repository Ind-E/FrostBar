use chrono::{DateTime, Local};
use iced::{
    Element,
    widget::{Container, MouseArea, container, text},
};

use crate::bar::{Message, MouseEvent};

pub struct TimeModule {
    pub time: DateTime<Local>,
    pub id: container::Id,
}

impl TimeModule {
    pub fn new() -> Self {
        Self {
            time: Local::now(),
            id: container::Id::unique(),
        }
    }

    pub fn to_widget<'a>(&self) -> Element<'a, Message> {
        let time = self.time.format("%I\n%M").to_string();
        MouseArea::new(Container::new(text(time).size(16)).id(self.id.clone()))
            .on_enter(Message::MouseEntered(MouseEvent::Tooltip(self.id.clone())))
            .on_exit(Message::MouseExited(MouseEvent::Tooltip(self.id.clone())))
            .into()
    }

    pub fn tooltip(&self) -> String {
        self.time.format("%a %b %-d\n%-m/%-d/%y").to_string()
    }
}

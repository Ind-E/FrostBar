use iced::{
    Element, Length,
    alignment::Horizontal,
    mouse::Interaction,
    padding::top,
    widget::{
        Column, Container, Image, MouseArea, Svg,
        container::{self},
        text,
    },
};
use itertools::Itertools;
use niri_ipc::{Action, WorkspaceReferenceArg};
use std::collections::HashMap;

use crate::{
    bar::{Message, MouseEvent},
    icon_cache::Icon,
    style::workspace_style,
};

pub struct Window {
    pub id: u64,
    pub icon: Option<Icon>,
}

impl<'a> Window {
    pub fn to_widget(&self, id: Option<container::Id>) -> Element<'a, Message> {
        let icon: Element<'a, Message> = match &self.icon {
            Some(Icon::Svg(handle)) => {
                Svg::new(handle.clone()).height(24).width(24).into()
            }
            Some(Icon::Raster(handle)) => {
                Image::new(handle.clone()).height(24).width(24).into()
            }
            Option::None => unreachable!(),
        };

        let container = Container::new(
            MouseArea::new(icon)
                .on_right_press(Message::NiriAction(Action::FocusWindow {
                    id: self.id,
                }))
                .on_enter(Message::MouseEntered(MouseEvent::Window(self.id)))
                .on_exit(Message::MouseExited(MouseEvent::Window(self.id))),
        );

        if let Some(id) = id {
            container.id(id).into()
        } else {
            container.into()
        }
    }
}

pub struct Workspace {
    pub output: Option<String>,
    pub idx: u8,
    pub id: u64,
    pub is_active: bool,
    pub windows: HashMap<u64, Window>,
}

impl<'a> Workspace {
    pub fn to_widget(
        &self,
        hovered: bool,
        window_ids: &HashMap<u64, container::Id>,
    ) -> Element<'a, Message> {
        Container::new(
            MouseArea::new(
                Container::new(
                    self.windows
                        .values()
                        .sorted_by(|w1, w2| w1.id.cmp(&w2.id))
                        .fold(
                            Column::new()
                                .align_x(Horizontal::Center)
                                .spacing(5)
                                .push(text(self.idx - 1).size(20)),
                            |col, w| {
                                col.push(
                                    w.to_widget(window_ids.get(&w.id).cloned()),
                                )
                            },
                        ),
                )
                .style(workspace_style(self.is_active, hovered))
                .padding(top(5).bottom(5))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
            )
            .on_press(Message::NiriAction(Action::FocusWorkspace {
                reference: WorkspaceReferenceArg::Id(self.id),
            }))
            .on_enter(Message::MouseEntered(MouseEvent::Workspace(self.idx)))
            .on_exit(Message::MouseExited(MouseEvent::Workspace(self.idx)))
            .interaction(Interaction::Pointer),
        )
        .into()
    }
}

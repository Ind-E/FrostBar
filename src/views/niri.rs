use iced::{
    Element, Length,
    alignment::Horizontal,
    mouse::Interaction,
    padding::top,
    widget::{
        Column, Container, Image, MouseArea, Svg, Text, column,
        container::{self},
        text::Shaping,
    },
};

use itertools::Itertools;
use niri_ipc::{Action, WindowLayout, WorkspaceReferenceArg};
use std::cmp::Ordering;

use crate::{
    Message, MouseEvent,
    config::Config,
    icon_cache::Icon,
    services::niri::{NiriEvent, NiriService, Window, Workspace},
    style::{styled_tooltip, workspace_style},
};

#[derive(Debug, Eq, PartialEq)]
enum Layout {
    Floating,
    Scrolling(usize, usize),
}

impl PartialOrd for Layout {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Layout {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Layout::Floating, Layout::Floating) => Ordering::Equal,
            (Layout::Floating, Layout::Scrolling(_, _)) => Ordering::Less,
            (Layout::Scrolling(_, _), Layout::Floating) => Ordering::Greater,
            (Layout::Scrolling(r1, c1), Layout::Scrolling(r2, c2)) => {
                r1.cmp(r2).then_with(|| c1.cmp(c2))
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct WindowView<'a> {
    container_id: container::Id,
    window: &'a Window,
}

impl<'a> From<&'a Window> for WindowView<'a> {
    fn from(window: &'a Window) -> Self {
        Self {
            container_id: container::Id::unique(),
            window,
        }
    }
}

impl<'a> WindowView<'a> {
    fn view(&self) -> Element<'a, Message> {
        let icon: Element<'a, Message> = match &self.window.icon {
            Some(Icon::Svg(handle)) => {
                Svg::new(handle.clone()).height(24).width(24).into()
            }
            Some(Icon::Raster(handle)) => {
                Image::new(handle.clone()).height(24).width(24).into()
            }
            Option::None => column![].into(),
        };

        let content =
            Container::new(MouseArea::new(icon).on_right_press(Message::NiriEvent(
                NiriEvent::Action(Action::FocusWindow { id: self.window.id }),
            )))
            .center_x(Length::Fill)
            .id(self.container_id.clone());

        let tooltip = Text::new(self.window.title.clone()).shaping(Shaping::Advanced);

        styled_tooltip(content, tooltip)
    }
}

struct WorkspaceView<'a> {
    workspace: &'a Workspace,
}

impl<'a> From<&'a Workspace> for WorkspaceView<'a> {
    fn from(workspace: &'a Workspace) -> Self {
        Self { workspace }
    }
}

impl<'a> WorkspaceView<'a> {
    fn view(&self, hovered: bool, radius: u16) -> Element<'a, Message> {
        Container::new(
            MouseArea::new(
                Container::new(
                    self.workspace.windows.values().sorted_unstable().fold(
                        Column::new()
                            .align_x(Horizontal::Center)
                            .spacing(5)
                            .push(Text::new(self.workspace.idx - 1).size(20)),
                        |col, w| col.push(<&Window as Into<WindowView>>::into(w).view()),
                    ),
                )
                .style(workspace_style(self.workspace.is_active, hovered, radius))
                .padding(top(5).bottom(5))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
            )
            .on_press(Message::NiriEvent(NiriEvent::Action(
                Action::FocusWorkspace {
                    reference: WorkspaceReferenceArg::Id(self.workspace.id),
                },
            )))
            .on_enter(Message::MouseEntered(MouseEvent::Workspace(
                self.workspace.id,
            )))
            .on_exit(Message::MouseExited(MouseEvent::Workspace(
                self.workspace.id,
            )))
            .interaction(Interaction::Pointer),
        )
        .into()
    }
}

impl From<WindowLayout> for Layout {
    fn from(layout: WindowLayout) -> Self {
        layout
            .pos_in_scrolling_layout
            .map_or(Layout::Floating, |l| Layout::Scrolling(l.0, l.1))
    }
}

pub struct NiriView {}

impl<'a> NiriView {
    pub fn view(
        &self,
        service: &'a NiriService,
        config: &Config,
    ) -> Element<'a, Message> {
        let ws = service
            .workspaces
            .iter()
            .sorted_by_key(|(_, ws)| ws.idx)
            .fold(Column::new(), |col, (_, ws)| {
                col.push(<&Workspace as Into<WorkspaceView>>::into(ws).view(
                    service.hovered_workspace_id.is_some_and(|id| id == ws.id),
                    config.layout.border_radius,
                ))
            })
            .align_x(Horizontal::Center)
            .spacing(10);
        ws.into()
    }
}

impl NiriView {
    pub fn new() -> Self {
        Self {}
    }
}

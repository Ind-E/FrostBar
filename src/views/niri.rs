use iced::{
    Alignment, Element, Length,
    mouse::Interaction,
    padding::{left, top},
    widget::{
        Column, Container, Image, MouseArea, Row, Svg, Text,
        container::{self},
        text::Shaping,
    },
};

use itertools::Itertools;
use niri_ipc::{Action, WorkspaceReferenceArg};

use crate::{
    Message, ModuleMessage, MouseEvent, config,
    icon_cache::Icon,
    services::niri::{NiriEvent, NiriService, Window, Workspace},
    style::{styled_tooltip, workspace_style},
    views::BarPosition,
};

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

#[profiling::all_functions]
impl<'a> WindowView<'a> {
    fn view(&self, layout: &config::Layout) -> Element<'a, Message> {
        let icon: Element<'a, Message> = match &self.window.icon {
            Some(Icon::Svg(handle)) => {
                Svg::new(handle.clone()).height(24).width(24).into()
            }
            Some(Icon::Raster(handle)) => {
                Image::new(handle.clone()).height(24).width(24).into()
            }
            Option::None => {
                let container = Container::new(
                    Text::new("?")
                        .size(20)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .center(),
                )
                .padding(5);
                if layout.anchor.vertical() {
                    container
                        .center_x(Length::Fill)
                        .height(layout.width - 10)
                        .into()
                } else {
                    container
                        .center_y(Length::Fill)
                        .width(layout.width - 10)
                        .into()
                }
            }
        };

        let mut content = Container::new(MouseArea::new(icon).on_right_press(
            Message::Msg(ModuleMessage::Niri(NiriEvent::Action(
                Action::FocusWindow { id: self.window.id },
            ))),
        ))
        .id(self.container_id.clone());
        if layout.anchor.vertical() {
            content = content.center_x(Length::Fill);
        } else {
            content = content.center_y(Length::Fill);
        }

        let tooltip =
            Text::new(self.window.title.clone()).shaping(Shaping::Advanced);

        styled_tooltip(content, tooltip, layout.anchor)
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

#[profiling::all_functions]
impl<'a> WorkspaceView<'a> {
    fn view(
        &self,
        hovered: bool,
        active_style: &config::ContainerStyle,
        hovered_style: &config::ContainerStyle,
        base_style: &config::ContainerStyle,
        offset: i8,
        layout: &config::Layout,
    ) -> Element<'a, Message> {
        let windows = if layout.anchor.vertical() {
            Container::new(
                self.workspace.windows.values().sorted_unstable().fold(
                    Column::new().align_x(Alignment::Center).spacing(5).push(
                        Text::new(self.workspace.idx as i8 + offset).size(20),
                    ),
                    |col, w| {
                        col.push(
                            <&Window as Into<WindowView>>::into(w).view(layout),
                        )
                    },
                ),
            )
            .padding(top(5).bottom(5))
            .width(Length::Fill)
            .align_x(Alignment::Center)
        } else {
            Container::new(
                self.workspace.windows.values().sorted_unstable().fold(
                    Row::new()
                        .align_y(Alignment::Center)
                        .spacing(5)
                        .padding(5)
                        .push(
                            Text::new(self.workspace.idx as i8 + offset)
                                .size(20),
                        ),
                    |col, w| {
                        col.push(
                            <&Window as Into<WindowView>>::into(w).view(layout),
                        )
                    },
                ),
            )
            .padding(left(5).right(5))
            .height(Length::Fill)
            .align_y(Alignment::Center)
        };

        let windows = windows.style(workspace_style(
            self.workspace.is_active,
            hovered,
            active_style,
            hovered_style,
            base_style,
        ));

        MouseArea::new(windows)
            .on_press(Message::Msg(ModuleMessage::Niri(NiriEvent::Action(
                Action::FocusWorkspace {
                    reference: WorkspaceReferenceArg::Id(self.workspace.id),
                },
            ))))
            .on_enter(Message::MouseEntered(MouseEvent::Workspace(
                self.workspace.id,
            )))
            .on_exit(Message::MouseExited(MouseEvent::Workspace(
                self.workspace.id,
            )))
            .interaction(Interaction::Pointer)
            .into()
    }
}

pub struct NiriView {
    config: config::Niri,
    pub position: BarPosition,
}

impl NiriView {
    pub fn new(config: config::Niri, position: BarPosition) -> Self {
        Self { config, position }
    }
}

#[profiling::all_functions]
impl<'a> NiriView {
    pub fn view(
        &self,
        service: &'a NiriService,
        layout: &config::Layout,
    ) -> Element<'a, Message> {
        if layout.anchor.vertical() {
            service
                .workspaces
                .iter()
                .sorted_by_key(|(_, ws)| ws.idx)
                .fold(Column::new(), |col, (_, ws)| {
                    col.push(
                        <&Workspace as Into<WorkspaceView>>::into(ws).view(
                            service
                                .hovered_workspace_id
                                .is_some_and(|id| id == ws.id),
                            &self.config.workspace_active_style,
                            &self.config.workspace_hovered_style,
                            &self.config.workspace_style,
                            self.config.workspace_offset,
                            layout,
                        ),
                    )
                })
                .align_x(Alignment::Center)
                .spacing(self.config.spacing)
                .into()
        } else {
            service
                .workspaces
                .iter()
                .sorted_by_key(|(_, ws)| ws.idx)
                .fold(Row::new(), |row, (_, ws)| {
                    row.push(
                        <&Workspace as Into<WorkspaceView>>::into(ws).view(
                            service
                                .hovered_workspace_id
                                .is_some_and(|id| id == ws.id),
                            &self.config.workspace_active_style,
                            &self.config.workspace_hovered_style,
                            &self.config.workspace_style,
                            self.config.workspace_offset,
                            layout,
                        ),
                    )
                })
                .align_y(Alignment::Center)
                .spacing(self.config.spacing)
                .into()
        }
    }
}

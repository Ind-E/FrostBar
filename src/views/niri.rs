use iced::{
    Alignment, Element, Length,
    mouse::Interaction,
    padding::{left, top},
    widget::{
        Column, Container, Image, MouseArea, Row, Svg, Text,
        container::{self},
    },
};

use itertools::Itertools;
use niri_ipc::{Action, WorkspaceReferenceArg};
use rustc_hash::FxHashMap;

use crate::{
    Message, ModuleMessage, MouseEvent, config,
    icon_cache::Icon,
    services::niri::{NiriEvent, NiriService, Window, Workspace},
    style::workspace_style,
    views::BarPosition,
};

#[derive(Debug, Eq, PartialEq)]
struct WindowView {
    id: container::Id,
}

#[profiling::all_functions]
impl WindowView {
    fn new() -> Self {
        Self {
            id: container::Id::unique(),
        }
    }

    fn render_tooltip<'a>(
        &self,
        window: &'a Window,
    ) -> Option<Element<'a, Message>> {
        use iced::widget::text::Shaping;
        Some(
            Text::new(window.title.clone())
                .shaping(Shaping::Advanced)
                .into(),
        )
    }

    fn view<'a>(
        &self,
        window: &'a Window,
        layout: &config::Layout,
    ) -> Element<'a, Message> {
        let icon_size = layout.width as f32 * 0.7;
        let placehdoler_text_size = icon_size * 0.6;
        let icon: Element<'a, Message> = match &window.icon {
            Some(Icon::Svg(handle)) => Svg::new(handle.clone())
                .height(icon_size)
                .width(icon_size)
                .into(),
            Some(Icon::Raster(handle)) => Image::new(handle.clone())
                .height(icon_size)
                .width(icon_size)
                .into(),
            _ => {
                let container = Container::new(
                    Text::new(window.title.chars().take(2).collect::<String>())
                        .size(placehdoler_text_size)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .center(),
                );
                if layout.anchor.vertical() {
                    container
                        .center_x(Length::Fill)
                        .height(icon_size)
                        .into()
                } else {
                    container
                        .center_y(Length::Fill)
                        .width(icon_size)
                        .into()
                }
            }
        };

        let mut content = Container::new(MouseArea::new(icon).on_right_press(
            Message::Msg(ModuleMessage::Niri(NiriEvent::Action(
                Action::FocusWindow { id: window.id },
            ))),
        ))
        .id(self.id.clone());

        if layout.anchor.vertical() {
            content = content.center_x(Length::Fill);
        } else {
            content = content.center_y(Length::Fill);
        }

        MouseArea::new(content)
            .on_enter(Message::OpenTooltip(self.id.clone()))
            .on_exit(Message::CloseTooltip(self.id.clone()))
            .into()
    }
}

struct WorkspaceView {
    window_views: FxHashMap<u64, WindowView>,
}

impl WorkspaceView {
    fn new() -> Self {
        Self {
            window_views: FxHashMap::default(),
        }
    }

    fn synchronize(&mut self, workspace: &Workspace) {
        self.window_views
            .retain(|id, _| workspace.windows.contains_key(id));
        for id in workspace.windows.keys() {
            self.window_views.entry(*id).or_insert_with(WindowView::new);
        }
    }

    fn view<'a>(
        &self,
        workspace: &'a Workspace,
        hovered: bool,
        active_style: &config::ContainerStyle,
        hovered_style: &config::ContainerStyle,
        base_style: &config::ContainerStyle,
        offset: i8,
        layout: &config::Layout,
    ) -> Element<'a, Message> {
        let windows = if layout.anchor.vertical() {
            Container::new(
                workspace.windows.values().sorted_unstable().fold(
                    Column::new()
                        .align_x(Alignment::Center)
                        .spacing(5)
                        .push(Text::new(workspace.idx as i8 + offset).size(20)),
                    |col, window| {
                        if let Some(view) = self.window_views.get(&window.id) {
                            col.push(view.view(window, layout))
                        } else {
                            col
                        }
                    },
                ),
            )
            .padding(top(5).bottom(5))
            .width(Length::Fill)
            .align_x(Alignment::Center)
        } else {
            Container::new(
                workspace.windows.values().sorted_unstable().fold(
                    Row::new()
                        .align_y(Alignment::Center)
                        .spacing(5)
                        .padding(5)
                        .push(Text::new(workspace.idx as i8 + offset).size(20)),
                    |row, window| {
                        if let Some(view) = self.window_views.get(&window.id) {
                            row.push(view.view(window, layout))
                        } else {
                            row
                        }
                    },
                ),
            )
            .padding(left(5).right(5))
            .height(Length::Fill)
            .align_y(Alignment::Center)
        };

        let windows = windows.style(workspace_style(
            workspace.is_active,
            hovered,
            active_style,
            hovered_style,
            base_style,
        ));

        MouseArea::new(windows)
            .on_press(Message::Msg(ModuleMessage::Niri(NiriEvent::Action(
                Action::FocusWorkspace {
                    reference: WorkspaceReferenceArg::Id(workspace.id),
                },
            ))))
            .on_enter(Message::MouseEntered(MouseEvent::Workspace(
                workspace.id,
            )))
            .on_exit(Message::MouseExited(MouseEvent::Workspace(workspace.id)))
            .interaction(Interaction::Pointer)
            .into()
    }
}

pub struct NiriView {
    config: config::Niri,
    pub position: BarPosition,
    workspace_views: FxHashMap<u64, WorkspaceView>,
}

#[profiling::all_functions]
impl NiriView {
    pub fn new(config: config::Niri, position: BarPosition) -> Self {
        Self {
            config,
            position,
            workspace_views: FxHashMap::default(),
        }
    }

    pub fn synchronize(&mut self, service: &NiriService) {
        self.workspace_views
            .retain(|id, _| service.workspaces.contains_key(id));

        for (id, workspace) in &service.workspaces {
            let ws_view = self
                .workspace_views
                .entry(*id)
                .or_insert_with(WorkspaceView::new);
            ws_view.synchronize(workspace);
        }
    }

    pub fn render_window_tooltip<'a>(
        &'a self,
        service: &'a NiriService,
        id: &container::Id,
    ) -> Option<Element<'a, Message>> {
        for (ws_id, ws_view) in &self.workspace_views {
            for (win_id, win_view) in &ws_view.window_views {
                if win_view.id == *id
                    && let Some(window) = service
                        .workspaces
                        .get(ws_id)
                        .and_then(|ws| ws.windows.get(win_id))
                {
                    return win_view.render_tooltip(window);
                }
            }
        }
        None
    }

    pub fn view<'a>(
        &'a self,
        service: &'a NiriService,
        layout: &config::Layout,
    ) -> Element<'a, Message> {
        if layout.anchor.vertical() {
            service
                .workspaces
                .iter()
                .sorted_by_key(|(_, ws)| ws.idx)
                .fold(Column::new(), |col, (_, ws)| {
                    if let Some(ws_view) = self.workspace_views.get(&ws.id) {
                        col.push(
                            ws_view.view(
                                ws,
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
                    } else {
                        col
                    }
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
                    if let Some(ws_view) = self.workspace_views.get(&ws.id) {
                        row.push(
                            ws_view.view(
                                ws,
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
                    } else {
                        row
                    }
                })
                .align_y(Alignment::Center)
                .spacing(self.config.spacing)
                .into()
        }
    }
}

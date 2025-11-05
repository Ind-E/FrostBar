use std::any::Any;

use iced::{
    Alignment, Length,
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
use rustc_hash::FxHashMap;

use super::service::{NiriEvent, Window, Workspace};
use crate::{
    Element, Message, MouseEvent,
    modules::{BarPosition, ModuleMsg, Modules, ViewTrait},
    other::{config, icon_cache::Icon},
    utils::style::workspace_style,
};

pub struct NiriView {
    config: config::Niri,
    position: BarPosition,
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
}

#[profiling::all_functions]
impl ViewTrait<Modules> for NiriView {
    fn view<'a>(
        &'a self,
        modules: &'a Modules,
        layout: &config::Layout,
    ) -> Element<'a> {
        let niri = &modules.niri;
        if layout.anchor.vertical() {
            niri.workspaces
                .iter()
                .sorted_unstable_by_key(|(_, ws)| ws.idx)
                .fold(Column::new(), |col, (_, ws)| {
                    if let Some(ws_view) = self.workspace_views.get(&ws.id) {
                        col.push(
                            ws_view.view(
                                ws,
                                niri.hovered_workspace_id
                                    .is_some_and(|id| id == ws.id),
                                &self
                                    .config
                                    .workspace_active_hovered_style_merged,
                                &self.config.workspace_active_style_merged,
                                &self.config.workspace_hovered_style_merged,
                                &self.config.workspace_default_style,
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
            niri.workspaces
                .iter()
                .sorted_unstable_by_key(|(_, ws)| ws.idx)
                .fold(Row::new(), |row, (_, ws)| {
                    if let Some(ws_view) = self.workspace_views.get(&ws.id) {
                        row.push(
                            ws_view.view(
                                ws,
                                niri.hovered_workspace_id
                                    .is_some_and(|id| id == ws.id),
                                &self
                                    .config
                                    .workspace_active_hovered_style_merged,
                                &self.config.workspace_active_style_merged,
                                &self.config.workspace_hovered_style_merged,
                                &self.config.workspace_default_style,
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

    fn position(&self) -> BarPosition {
        self.position
    }

    fn tooltip<'a>(
        &'a self,
        service: &'a Modules,
        id: &container::Id,
    ) -> Option<Element<'a>> {
        let service = &service.niri;
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

    fn synchronize(&mut self, modules: &Modules) {
        let service = &modules.niri;
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

    fn as_any(&self) -> &dyn Any {
        self
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
        active_hovered_style: &'a config::ContainerStyle,
        active_style: &'a config::ContainerStyle,
        hovered_style: &'a config::ContainerStyle,
        base_style: &'a config::ContainerStyle,
        offset: i8,
        layout: &config::Layout,
    ) -> Element<'a> {
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
            active_hovered_style,
            active_style,
            hovered_style,
            base_style,
        ));

        MouseArea::new(windows)
            .on_press(Message::Module(ModuleMsg::Niri(NiriEvent::Action(
                Action::FocusWorkspace {
                    reference: WorkspaceReferenceArg::Id(workspace.id),
                },
            ))))
            .on_enter(Message::Module(ModuleMsg::MouseEntered(
                MouseEvent::Workspace(workspace.id),
            )))
            .on_exit(Message::Module(ModuleMsg::MouseExited(
                MouseEvent::Workspace(workspace.id),
            )))
            .interaction(Interaction::Pointer)
            .into()
    }
}

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

    fn render_tooltip<'a>(&self, window: &'a Window) -> Option<Element<'a>> {
        Some(Text::new(&window.title).shaping(Shaping::Advanced).into())
    }

    fn view<'a>(
        &self,
        window: &'a Window,
        layout: &config::Layout,
    ) -> Element<'a> {
        let icon_size = layout.width as f32 * 0.7;
        let placehdoler_text_size = icon_size * 0.6;
        let icon: Element<'a> = match &window.icon {
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
                    container.center_x(Length::Fill).height(icon_size).into()
                } else {
                    container.center_y(Length::Fill).width(icon_size).into()
                }
            }
        };

        let mut content = Container::new(MouseArea::new(icon).on_right_press(
            Message::Module(ModuleMsg::Niri(NiriEvent::Action(
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

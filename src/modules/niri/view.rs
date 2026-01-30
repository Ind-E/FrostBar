use super::service::{NiriEvent, Window, Workspace};
use crate::{
    Element, Message, MouseEvent,
    config::{self, NiriWindowStyle},
    icon_cache::Icon,
    modules::{
        BarPosition, ModuleMsg, Modules, ViewTrait, niri::service::NiriService,
    },
    utils::style::{window_style, workspace_style},
};
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
use std::any::Any;

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
        let service = modules.niri.as_ref().expect("niri should not be None");
        if layout.anchor.vertical() {
            service
                .workspaces
                .iter()
                .sorted_unstable_by_key(|(_, ws)| ws.idx)
                .fold(Column::new(), |col, (_, ws)| {
                    if let Some(ws_view) = self.workspace_views.get(&ws.id) {
                        col.push(
                            ws_view.view(
                                service,
                                ws,
                                service
                                    .hovered_workspace_id
                                    .is_some_and(|id| id == ws.id),
                                &self.config,
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
                .sorted_unstable_by_key(|(_, ws)| ws.idx)
                .fold(Row::new(), |row, (_, ws)| {
                    if let Some(ws_view) = self.workspace_views.get(&ws.id) {
                        row.push(
                            ws_view.view(
                                service,
                                ws,
                                service
                                    .hovered_workspace_id
                                    .is_some_and(|id| id == ws.id),
                                &self.config,
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
        modules: &'a Modules,
        id: &container::Id,
    ) -> Option<Element<'a>> {
        let service = modules.niri.as_ref().expect("niri should not be None");
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
        let service = modules.niri.as_ref().expect("niri should not be None");
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
        niri: &NiriService,
        workspace: &'a Workspace,
        hovered: bool,
        config: &'a config::Niri,
        layout: &config::Layout,
    ) -> Element<'a> {
        let windows = if layout.anchor.vertical() {
            Container::new(
                workspace.windows.values().sorted_unstable().fold(
                    Column::new().align_x(Alignment::Center).push(
                        Text::new(
                            workspace.idx as i8 + config.workspace_offset,
                        )
                        .size(20),
                    ),
                    |col, window| {
                        if let Some(view) = self.window_views.get(&window.id) {
                            col.push(view.view(
                                window,
                                niri.focused_window_id == Some(window.id),
                                &config.window_style,
                                layout,
                            ))
                        } else {
                            col
                        }
                    },
                ),
            )
            .padding(top(3).bottom(3))
            .width(Length::Fill)
            .align_x(Alignment::Center)
        } else {
            Container::new(
                workspace.windows.values().sorted_unstable().fold(
                    Row::new()
                        .align_y(Alignment::Center)
                        .spacing(5)
                        .padding(5)
                        .push(
                            Text::new(
                                workspace.idx as i8 + config.workspace_offset,
                            )
                            .size(20),
                        ),
                    |row, window| {
                        if let Some(view) = self.window_views.get(&window.id) {
                            row.push(view.view(
                                window,
                                niri.focused_window_id == Some(window.id),
                                &config.window_style,
                                layout,
                            ))
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
            &config.workspace_style,
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
        Some(
            Text::new(
                if let Some(title) = &window.title
                    && !title.is_empty()
                {
                    title
                } else if let Some(app_id) = &window.app_id
                    && !app_id.is_empty()
                {
                    app_id
                } else {
                    "N/A"
                },
            )
            .shaping(Shaping::Advanced)
            .into(),
        )
    }

    fn view<'a>(
        &self,
        window: &'a Window,
        focused: bool,
        style: &'a NiriWindowStyle,
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
                    Text::new(
                        if let Some(title) = &window.title
                            && !title.is_empty()
                        {
                            title
                        } else if let Some(app_id) = &window.app_id
                            && !app_id.is_empty()
                        {
                            app_id
                        } else {
                            "N/A"
                        }
                        .chars()
                        .take(2)
                        .collect::<String>(),
                    )
                    .size(placehdoler_text_size)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center(),
                );
                if layout.anchor.vertical() {
                    container.center_x(Length::Shrink).height(icon_size).into()
                } else {
                    container.center_y(Length::Shrink).width(icon_size).into()
                }
            }
        };

        let mut content = Container::new(MouseArea::new(icon).on_right_press(
            Message::Module(ModuleMsg::Niri(NiriEvent::Action(
                Action::FocusWindow { id: window.id },
            ))),
        ))
        .padding(3)
        .style(window_style(focused, style))
        .id(self.id.clone());

        if layout.anchor.vertical() {
            content = content.align_x(Alignment::Center);
        } else {
            content = content.align_y(Alignment::Center);
        }

        MouseArea::new(content)
            .on_enter(Message::OpenTooltip(self.id.clone()))
            .on_exit(Message::CloseTooltip(self.id.clone()))
            .into()
    }
}

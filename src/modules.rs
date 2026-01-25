use crate::{
    Element, Message, MouseEvent,
    config::{self, Config, ConfigModule, MouseBinds},
    icon_cache::IconCache,
    modules::audio_visualizer::{
        service::AudioVisualizerService, view::AudioVisualizerView,
    },
};
use battery::{service::BatteryService, view::BatteryView};
use chrono::{DateTime, Local};
use iced::{
    Color, Task,
    mouse::ScrollDelta,
    widget::{MouseArea, container, image},
};
use label::LabelView;
use mpris::{
    service::{MprisEvent, MprisService},
    view::MprisView,
};
use niri::{
    service::{NiriEvent, NiriService},
    view::NiriView,
};
use std::any::Any;
// use system_tray::{service::SystemTrayService, view::SystemTrayView};
use time::{service::TimeService, view::TimeView};

pub mod audio_visualizer;
pub mod battery;
pub mod label;
pub mod mpris;
pub mod niri;
// pub mod system_tray;
pub mod time;

#[derive(Debug, Clone)]
pub enum ModuleMsg {
    Tick(DateTime<Local>),
    Niri(NiriEvent),
    AudioSample(Vec<f32>),
    AudioVisualizerGradientUpdate(Option<Vec<Color>>),
    AudioVisualizerTimer,
    PlayerArtUpdate(String, Option<(image::Handle, Option<Vec<Color>>)>),
    Mpris(MprisEvent),
    // Systray(system_tray::service::Event),
    SynchronizeAll,
    MouseEntered(MouseEvent),
    MouseExited(MouseEvent),
    NoOp,
}

pub type View = Box<dyn ViewTrait<Modules>>;

pub struct Modules {
    pub battery: BatteryService,
    pub audio_visualizer: AudioVisualizerService,
    pub mpris: MprisService,
    pub time: TimeService,
    pub niri: NiriService,
    // pub systray: SystemTrayService,
    pub views: Vec<View>,
}

#[profiling::all_functions]
impl Modules {
    pub fn new(icon_cache: IconCache) -> Self {
        Self {
            battery: BatteryService::new(),
            audio_visualizer: AudioVisualizerService::new(),
            mpris: MprisService::new(),
            time: TimeService::new(),
            niri: NiriService::new(icon_cache.clone()),
            // systray: SystemTrayService::new(icon_cache),
            views: Vec::new(),
        }
    }

    pub fn update_from_config(&mut self, config: &mut Config) {
        self.views.clear();

        for (module, position) in config.modules.drain(..) {
            match module {
                ConfigModule::Battery(c) => {
                    self.views.push(Box::new(BatteryView::new(c, position)));
                }
                ConfigModule::AudioVisualizer(c) => {
                    self.views
                        .push(Box::new(AudioVisualizerView::new(c, position)));
                }
                ConfigModule::Time(c) => {
                    self.views.push(Box::new(TimeView::new(c, position)));
                }
                ConfigModule::Mpris(c) => {
                    self.views.push(Box::new(MprisView::new(c, position)));
                }
                ConfigModule::Niri(c) => {
                    self.views.push(Box::new(NiriView::new(*c, position)));
                }
                ConfigModule::Label(c) => {
                    self.views.push(Box::new(LabelView::new(c, position)));
                }
                ConfigModule::SystemTray(_c) => {
                    //     self.views.push(Box::new(SystemTrayView::new(c, position)));
                }
            }
        }
    }

    pub fn render_views<'a>(
        &'a self,
        layout: &'a config::Layout,
    ) -> impl Iterator<Item = (Element<'a>, BarPosition)> + 'a {
        self.views
            .iter()
            .map(move |v| (v.view(self, layout), v.position()))
    }

    pub fn render_tooltip_for_id<'a>(
        &'a self,
        id: &container::Id,
    ) -> Option<Element<'a>> {
        self.views.iter().find_map(|view| view.tooltip(self, id))
    }

    pub fn render_menu_for_id<'a>(
        &'a self,
        id: &container::Id,
    ) -> Option<Element<'a>> {
        self.views.iter().find_map(|view| view._menu(self, id))
    }

    #[must_use]
    pub fn update(&mut self, message: ModuleMsg) -> ModuleAction {
        match message {
            ModuleMsg::MouseEntered(event) => {
                let MouseEvent::Workspace(id) = event;
                self.niri.hovered_workspace_id = Some(id);
            }
            ModuleMsg::MouseExited(_event) => {
                self.niri.hovered_workspace_id = None;
            }
            ModuleMsg::Tick(time) => {
                self.time.update(time);
                self.battery.fetch_battery_info();
                self.synchronize_views_filtered(|view| {
                    view.as_any().is::<TimeView>()
                });
            }
            ModuleMsg::Niri(event) => {
                let task = self.niri.update(event);
                self.synchronize_views_filtered(|view| {
                    view.as_any().is::<NiriView>()
                });
                return task;
            }
            ModuleMsg::AudioSample(sample) => {
                self.audio_visualizer.update(sample);
            }
            ModuleMsg::AudioVisualizerGradientUpdate(gradient) => {
                self.audio_visualizer.update_gradient(gradient);
            }
            ModuleMsg::Mpris(event) => {
                let task = self.mpris.update(event);
                self.synchronize_views_filtered(|view| {
                    view.as_any().is::<MprisView>()
                });
                return task;
            }
            ModuleMsg::PlayerArtUpdate(name, art) => {
                if let Some(player) = self.mpris.players.get_mut(&name)
                    && let Some((art, gradient)) = art
                {
                    player.art = Some(art);
                    player.colors.clone_from(&gradient);
                    if player.status == "Playing" {
                        let captured_colors = gradient;
                        return ModuleAction::Task(iced::Task::perform(
                            async move { captured_colors },
                            ModuleMsg::AudioVisualizerGradientUpdate,
                        ));
                    }
                }
            }
            // ModuleMsg::Systray(event) => {
            //     self.systray.update(event);
            //     self.synchronize_views_filtered(|view| {
            //         view.as_any().is::<SystemTrayView>()
            //     });
            // }
            ModuleMsg::SynchronizeAll => {
                self.synchronize_views();
            }
            ModuleMsg::AudioVisualizerTimer => {
                self.audio_visualizer.timer_update();
            }
            ModuleMsg::NoOp => {}
        }
        ModuleAction::None
    }

    #[allow(clippy::deref_addrof, clippy::ref_as_ptr)]
    pub fn synchronize_views(&mut self) {
        for i in 0..self.views.len() {
            let view = unsafe { &mut *(&raw mut self.views[i]) };
            view.synchronize(unsafe { &*(self as *const _) });
        }
    }

    #[allow(clippy::deref_addrof, clippy::ref_as_ptr)]
    fn synchronize_views_filtered(&mut self, filter: fn(&View) -> bool) {
        for i in 0..self.views.len() {
            let view = unsafe { &mut *(&raw mut self.views[i]) };
            if filter(view) {
                view.synchronize(unsafe { &*(self as *const _) });
            }
        }
    }
}

pub enum ModuleAction {
    Task(Task<ModuleMsg>),
    None,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct BarPosition {
    pub idx: usize,
    pub align: BarAlignment,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum BarAlignment {
    Start,
    Middle,
    End,
}

pub trait ViewTrait<M>: Any {
    fn view<'a>(
        &'a self,
        modules: &'a M,
        layout: &'a config::Layout,
    ) -> Element<'a>;

    fn position(&self) -> BarPosition;

    fn tooltip<'a>(
        &'a self,
        _modules: &'a M,
        _id: &container::Id,
    ) -> Option<Element<'a>> {
        None
    }

    fn _menu<'a>(
        &'a self,
        _modules: &'a M,
        _id: &container::Id,
    ) -> Option<Element<'a>> {
        None
    }

    fn synchronize(&mut self, _modules: &M) {}

    fn as_any(&self) -> &dyn Any;
}

#[profiling::function]
pub fn mouse_binds<'a>(
    element: impl Into<Element<'a>>,
    binds: &'a MouseBinds,
    tooltip_id: Option<container::Id>,
) -> Element<'a> {
    let mut mouse_area = MouseArea::new(element);

    if let Some(id) = tooltip_id {
        mouse_area = mouse_area
            .on_enter(Message::OpenTooltip(id.clone()))
            .on_exit(Message::CloseTooltip(id));
    }

    if let Some(left) = &binds.mouse_left {
        mouse_area = mouse_area.on_release(left.clone());
    }

    if let Some(double) = &binds.double_click {
        mouse_area = mouse_area.on_double_click(double.clone());
    }

    if let Some(right) = &binds.mouse_right {
        mouse_area = mouse_area.on_right_release(right.clone());
    }

    if let Some(middle) = &binds.mouse_middle {
        mouse_area = mouse_area.on_middle_release(middle.clone());
    }

    if let Some(ref scroll) = binds.scroll {
        mouse_area = mouse_area.on_scroll(|delta| {
            let (x, y) = match delta {
                ScrollDelta::Lines { x, y } | ScrollDelta::Pixels { x, y } => {
                    (x, y)
                }
            };

            if y > 0.0
                && let Some(up) = scroll.up.clone()
            {
                up
            } else if y < 0.0
                && let Some(down) = scroll.down.clone()
            {
                down
            } else if x < 0.0
                && let Some(right) = scroll.right.clone()
            {
                right
            } else if x > 0.0
                && let Some(left) = scroll.left.clone()
            {
                left
            } else {
                Message::NoOp
            }
        });
    }

    mouse_area.into()
}

#[profiling::function]
pub fn process_command(cmd: &config::Command) -> Message {
    if cmd.args.is_empty() {
        Message::NoOp
    } else if let Some(sh) = cmd.sh
        && sh
    {
        Message::Command(CommandSpec {
            command: String::from("sh"),
            args: Some(vec![String::from("-c"), cmd.args[0].clone()]),
        })
    } else {
        Message::Command(CommandSpec {
            command: cmd.args[0].clone(),
            args: cmd.args.get(1..).map(<[String]>::to_vec),
        })
    }
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub command: String,
    pub args: Option<Vec<String>>,
}

impl std::fmt::Display for CommandSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(args) = self.args.as_ref()
            && !args.is_empty()
        {
            if args[0] == "-c" {
                write!(f, "{}", args[1..].join(" "))
            } else {
                write!(f, "{} {}", self.command, args.join(" "))
            }
        } else {
            write!(f, "{}", self.command)
        }
    }
}

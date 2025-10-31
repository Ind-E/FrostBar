use chrono::{DateTime, Local};
use iced::{
    Color, Element, Task,
    widget::{container, image},
};

use crate::{
    Message as CrateMessage, MouseEvent,
    config::{self, Config, ConfigModule},
    icon_cache::IconCache,
    services::{
        battery::BatteryService,
        cava::CavaService,
        mpris::{MprisEvent, MprisService},
        niri::{NiriEvent, NiriService},
        systray::{self, Systray},
        time::TimeService,
    },
    views::{
        BarAlignment, BarPosition, ViewTrait, battery::BatteryView,
        cava::CavaView, label::LabelView, mpris::MprisView, niri::NiriView,
        systray::SystrayView, time::TimeView,
    },
};

#[derive(Debug, Clone)]
pub enum Message {
    Tick(DateTime<Local>),
    Niri(NiriEvent),
    CavaUpdate(Option<String>),
    CavaColorUpdate(Option<Vec<Color>>),
    PlayerArtUpdate(String, Option<(image::Handle, Option<Vec<Color>>)>),
    Mpris(MprisEvent),
    Systray(systray::Event),
    SynchronizeAll,
    MouseEntered(MouseEvent),
    MouseExited(MouseEvent),
    NoOp,
}

pub type View = Box<dyn ViewTrait<Modules>>;

pub struct Modules {
    pub battery: BatteryService,
    pub cava: CavaService,
    pub mpris: MprisService,
    pub time: TimeService,
    pub niri: NiriService,
    pub systray: Systray,
    pub views: Vec<View>,
}

#[profiling::all_functions]
impl Modules {
    pub fn new(icon_cache: IconCache) -> Self {
        Self {
            battery: BatteryService::new(),
            cava: CavaService::new(),
            mpris: MprisService::new(),
            time: TimeService::new(),
            niri: NiriService::new(icon_cache.clone()),
            systray: Systray::new(icon_cache),
            views: Vec::new(),
        }
    }

    pub fn update_from_config(&mut self, config: &mut Config) {
        self.views.clear();

        let mut process_section =
            |module_configs: &mut Vec<ConfigModule>, align: BarAlignment| {
                for (idx, module_config) in module_configs.drain(..).enumerate()
                {
                    let position = BarPosition { idx, align };

                    match module_config {
                        ConfigModule::Battery(c) => {
                            self.views
                                .push(Box::new(BatteryView::new(c, position)));
                        }
                        ConfigModule::Cava(c) => {
                            self.views
                                .push(Box::new(CavaView::new(c, position)));
                        }
                        ConfigModule::Mpris(c) => {
                            self.views
                                .push(Box::new(MprisView::new(c, position)));
                        }
                        ConfigModule::Time(c) => {
                            self.views
                                .push(Box::new(TimeView::new(c, position)));
                        }
                        ConfigModule::Label(c) => {
                            self.views
                                .push(Box::new(LabelView::new(c, position)));
                        }
                        ConfigModule::Niri(c) => {
                            self.views
                                .push(Box::new(NiriView::new(c, position)));
                        }
                    }
                }
            };

        process_section(&mut config.start.modules, BarAlignment::Start);
        process_section(&mut config.middle.modules, BarAlignment::Middle);
        process_section(&mut config.end.modules, BarAlignment::End);

        self.views.push(Box::new(SystrayView {}));
    }

    pub fn render_views<'a>(
        &'a self,
        layout: &'a config::Layout,
    ) -> impl Iterator<Item = (Element<'a, CrateMessage>, BarPosition)> + 'a
    {
        self.views
            .iter()
            .map(move |v| (v.view(self, layout), v.position()))
    }

    pub fn render_tooltip_for_id<'a>(
        &'a self,
        id: &container::Id,
    ) -> Option<Element<'a, CrateMessage>> {
        self.views.iter().find_map(|view| view.tooltip(self, id))
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> ModuleAction {
        match message {
            Message::MouseEntered(event) => {
                let MouseEvent::Workspace(id) = event;
                self.niri.hovered_workspace_id = Some(id);
            }
            Message::MouseExited(_event) => {
                self.niri.hovered_workspace_id = None;
            }
            Message::Tick(time) => {
                self.time.handle_event(time);
                self.battery.fetch_battery_info();
                self.synchronize_views_filtered(|view| {
                    view.as_any().is::<TimeView>()
                });
            }
            Message::Niri(event) => {
                let task = self.niri.handle_event(event);
                self.synchronize_views_filtered(|view| {
                    view.as_any().is::<NiriView>()
                });
                return task;
            }
            Message::CavaUpdate(event) => {
                self.cava.handle_event(event);
            }
            Message::CavaColorUpdate(gradient) => {
                self.cava.update_gradient(gradient);
            }
            Message::Mpris(event) => {
                let task = self.mpris.handle_event(event);
                self.synchronize_views_filtered(|view| {
                    view.as_any().is::<MprisView>()
                });
                return task;
            }
            Message::PlayerArtUpdate(name, art) => {
                if let Some(player) = self.mpris.players.get_mut(&name)
                    && let Some((art, colors)) = art
                {
                    player.art = Some(art);
                    player.colors.clone_from(&colors);
                    self.cava.update_gradient(colors);
                }
            }
            Message::Systray(event) => {
                self.systray.handle_event(event);
            }
            Message::SynchronizeAll => {
                self.synchronize_views();
            }
            Message::NoOp => {}
        }
        ModuleAction::None
    }

    fn synchronize_views(&mut self) {
        for i in 0..self.views.len() {
            let view = unsafe { &mut *(&raw mut self.views[i]) };
            view.synchronize(unsafe { &*(self as *const _) });
        }
    }

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
    Task(Task<Message>),
    None,
}

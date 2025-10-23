use iced::{
    Color, Element, Subscription, Task,
    widget::{container, image},
};
use rustc_hash::FxHashMap;

use crate::{
    Message, ModuleMessage, MouseEvent,
    config::{self, Config, ConfigModule},
    icon_cache::IconCache,
    services::{
        Service, battery::BatteryService, cava::CavaService,
        mpris::MprisService, niri::NiriService, time::TimeService,
    },
    views::{
        BarAlignment, BarPosition, battery::BatteryView, cava::CavaView,
        label::LabelView, mpris::MprisView, niri::NiriView, time::TimeView,
    },
};
use std::sync::{Arc, Mutex};

pub enum Module {
    Battery {
        service: BatteryService,
        views: Vec<BatteryView>,
    },
    Cava {
        service: CavaService,
        views: Vec<CavaView>,
    },
    Label {
        views: Vec<LabelView>,
    },
    Mpris {
        service: MprisService,
        views: Vec<MprisView>,
    },
    Niri {
        service: NiriService,
        views: Vec<NiriView>,
    },
    Time {
        service: TimeService,
        views: Vec<TimeView>,
    },
}

pub trait ModuleDyn {
    fn subscription(&self) -> Option<Subscription<Message>>;

    fn render_views<'a>(
        &'a self,
        layout: &'a config::Layout,
    ) -> Box<dyn Iterator<Item = (Element<'a, Message>, BarPosition)> + 'a>;

    fn render_tooltip<'a>(
        &'a self,
        id: &container::Id,
    ) -> Option<Element<'a, Message>>;
}

impl ModuleDyn for Module {
    fn subscription(&self) -> Option<Subscription<Message>> {
        match self {
            Module::Time { .. } => Some(TimeService::subscription()),
            Module::Battery { .. } => Some(BatteryService::subscription()),
            Module::Niri { .. } => Some(NiriService::subscription()),
            Module::Mpris { .. } => Some(MprisService::subscription()),
            Module::Cava { .. } => Some(CavaService::subscription()),
            Module::Label { .. } => None,
        }
    }

    fn render_views<'a>(
        &'a self,
        layout: &'a config::Layout,
    ) -> Box<dyn Iterator<Item = (Element<'a, Message>, BarPosition)> + 'a>
    {
        match self {
            Module::Battery { service, views } => Box::new(
                views
                    .iter()
                    .map(move |v| (v.view(service, layout), v.position)),
            ),
            Module::Cava { service, views } => Box::new(
                views
                    .iter()
                    .map(move |v| (v.view(service, layout), v.position)),
            ),
            Module::Mpris { service, views } => Box::new(
                views
                    .iter()
                    .map(move |v| (v.view(service, layout), v.position)),
            ),
            Module::Niri { service, views } => Box::new(
                views
                    .iter()
                    .map(move |v| (v.view(service, layout), v.position)),
            ),
            Module::Time { service, views } => Box::new(
                views
                    .iter()
                    .map(move |v| (v.view(service, layout), v.position)),
            ),
            Module::Label { views } => Box::new(
                views.iter().map(move |v| (v.view(layout), v.position)),
            ),
        }
    }

    fn render_tooltip<'a>(
        &'a self,
        id: &container::Id,
    ) -> Option<Element<'a, Message>> {
        match self {
            Module::Label { views, .. } => views.iter().find_map(|v| {
                if v.id == *id {
                    v.render_tooltip()
                } else {
                    None
                }
            }),
            Module::Battery { service, views } => views.iter().find_map(|v| {
                if v.id == *id {
                    v.render_tooltip(service)
                } else {
                    None
                }
            }),
            Module::Mpris { service, views } => views
                .iter()
                .find_map(|v| v.render_player_tooltip(service, id)),
            Module::Time { service, views } => views.iter().find_map(|v| {
                if v.id == *id {
                    v.render_tooltip(service)
                } else {
                    None
                }
            }),
            Module::Niri { service, views } => views
                .iter()
                .find_map(|v| v.render_window_tooltip(service, id)),
            Module::Cava { .. } => None,
        }
    }
}

pub struct Modules {
    inner: FxHashMap<&'static str, Module>,
}

impl Modules {
    pub fn new(
        icon_cache: Arc<Mutex<IconCache>>,
        icon_theme: Option<String>,
    ) -> Self {
        let mut modules = FxHashMap::default();

        modules.insert(
            "Battery",
            Module::Battery {
                service: BatteryService::new(),
                views: Vec::new(),
            },
        );
        modules.insert(
            "Cava",
            Module::Cava {
                service: CavaService::new(),
                views: Vec::new(),
            },
        );
        modules.insert(
            "Mpris",
            Module::Mpris {
                service: MprisService::new(),
                views: Vec::new(),
            },
        );
        modules.insert(
            "Time",
            Module::Time {
                service: TimeService::new(),
                views: Vec::new(),
            },
        );
        modules.insert("Label", Module::Label { views: Vec::new() });
        modules.insert(
            "Niri",
            Module::Niri {
                service: NiriService::new(icon_cache, icon_theme),
                views: Vec::new(),
            },
        );

        Self { inner: modules }
    }

    pub fn update_from_config(&mut self, config: &mut Config) {
        for module in self.inner.values_mut() {
            match module {
                Module::Battery { views, .. } => views.clear(),
                Module::Cava { views, .. } => views.clear(),
                Module::Mpris { views, .. } => views.clear(),
                Module::Niri { views, .. } => views.clear(),
                Module::Time { views, .. } => views.clear(),
                Module::Label { views } => views.clear(),
            }
        }

        let mut process_section =
            |module_configs: &mut Vec<ConfigModule>, align: BarAlignment| {
                for (idx, module_config) in module_configs.drain(..).enumerate()
                {
                    let position = BarPosition { idx, align };

                    match module_config {
                        ConfigModule::Battery(c) => {
                            if let Some(Module::Battery { views, .. }) =
                                self.inner.get_mut("Battery")
                            {
                                views.push(BatteryView::new(c, position));
                            }
                        }
                        ConfigModule::Cava(c) => {
                            if let Some(Module::Cava { views, .. }) =
                                self.inner.get_mut("Cava")
                            {
                                views.push(CavaView::new(c, position));
                            }
                        }
                        ConfigModule::Mpris(c) => {
                            if let Some(Module::Mpris { views, .. }) =
                                self.inner.get_mut("Mpris")
                            {
                                views.push(MprisView::new(c, position));
                            }
                        }
                        ConfigModule::Time(c) => {
                            if let Some(Module::Time { views, .. }) =
                                self.inner.get_mut("Time")
                            {
                                views.push(TimeView::new(c, position));
                            }
                        }
                        ConfigModule::Label(c) => {
                            if let Some(Module::Label { views }) =
                                self.inner.get_mut("Label")
                            {
                                views.push(LabelView::new(c, position));
                            }
                        }
                        ConfigModule::Niri(c) => {
                            if let Some(Module::Niri { views, .. }) =
                                self.inner.get_mut("Niri")
                            {
                                views.push(NiriView::new(c, position));
                            }
                        }
                    }
                }
            };

        process_section(&mut config.start.modules, BarAlignment::Start);
        process_section(&mut config.middle.modules, BarAlignment::Middle);
        process_section(&mut config.end.modules, BarAlignment::End);
    }

    pub fn subscriptions(
        &self,
    ) -> impl Iterator<Item = Subscription<Message>> + '_ {
        self.inner.values().filter_map(ModuleDyn::subscription)
    }

    pub fn render_views<'a>(
        &'a self,
        layout: &'a config::Layout,
    ) -> impl Iterator<Item = (Element<'a, Message>, BarPosition)> + 'a {
        self.inner
            .values()
            .flat_map(move |module| module.render_views(layout))
    }

    pub fn render_tooltip_for_id<'a>(
        &'a self,
        id: &container::Id,
    ) -> Option<Element<'a, Message>> {
        self.inner
            .values()
            .find_map(|module| module.render_tooltip(id))
    }

    pub fn handle_cava_color_update(
        &mut self,
        gradient: Option<Vec<Color>>,
    ) -> Task<Message> {
        if let Some(Module::Cava { service, .. }) = self.inner.get_mut("Cava") {
            service.update_gradient(gradient)
        } else {
            Task::none()
        }
    }

    pub fn handle_async_mpris_art_update(
        &mut self,
        player_name: &str,
        new_art: Option<(image::Handle, Option<Vec<Color>>)>,
    ) -> Task<Message> {
        if let Some(Module::Mpris { service, .. }) = self.inner.get_mut("Mpris")
            && let Some(player) = service.players.get_mut(player_name)
            && let Some((art, colors)) = new_art
        {
            player.art = Some(art);
            player.colors.clone_from(&colors);
            if let Some(Module::Cava { service, .. }) =
                self.inner.get_mut("Cava")
            {
                return service.update_gradient(colors);
            }
        }
        Task::none()
    }

    pub fn handle_mouse_entered(&mut self, event: MouseEvent) -> Task<Message> {
        let MouseEvent::Workspace(id) = event;
        if let Some(Module::Niri { service, .. }) = self.inner.get_mut("Niri") {
            service.hovered_workspace_id = Some(id);
        }
        Task::none()
    }

    pub fn handle_mouse_exited(&mut self, _event: MouseEvent) -> Task<Message> {
        if let Some(Module::Niri { service, .. }) = self.inner.get_mut("Niri") {
            service.hovered_workspace_id = None;
        }
        Task::none()
    }

    pub fn handle_event(
        &mut self,
        module_message: ModuleMessage,
    ) -> iced::Task<Message> {
        match module_message {
            ModuleMessage::Tick(time) => {
                if let Some(Module::Time { service, .. }) =
                    self.inner.get_mut("Time")
                {
                    return service.handle_event(time);
                }
            }
            ModuleMessage::UpdateBattery(event) => {
                if let Some(Module::Battery { service, .. }) =
                    self.inner.get_mut("Battery")
                {
                    return service.handle_event(event);
                }
            }
            ModuleMessage::Niri(event) => {
                if let Some(Module::Niri { service, views }) =
                    self.inner.get_mut("Niri")
                {
                    let task = service.handle_event(event);
                    for view in views.iter_mut() {
                        view.synchronize(service);
                    }
                    return task;
                }
            }
            ModuleMessage::CavaUpdate(event) => {
                if let Some(Module::Cava { service, .. }) =
                    self.inner.get_mut("Cava")
                {
                    return service.handle_event(event);
                }
            }
            ModuleMessage::Mpris(event) => {
                if let Some(Module::Mpris { service, views }) =
                    self.inner.get_mut("Mpris")
                {
                    let task = service.handle_event(event);
                    for view in views.iter_mut() {
                        view.synchronize(service);
                    }
                    return task;
                }
            }
        }
        iced::Task::none()
    }
}

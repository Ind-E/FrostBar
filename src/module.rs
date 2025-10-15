use iced::{Color, Element, Subscription, Task, widget::image};
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
}

pub struct Modules {
    inner: FxHashMap<&'static str, Module>,
}

macro_rules! impl_handle_event_for_modules {
    (
        // The first argument is the match target (e.g., `module_message`)
        $self:ident, $event:ident,
        // The rest of the arguments are the dispatch mapping:
        // MessageVariant(payload) => ModuleVariant
        $( $msg_variant:ident ( $payload:ident ) => $mod_variant:ident ),*
    ) => {
        /// Handles a standard `ModuleMessage` by dispatching it to the
        /// correct service's `handle_event` method.
        pub fn handle_event(&mut $self, $event: ModuleMessage) -> iced::Task<Message> {
            match $event {
                $(
                    ModuleMessage::$msg_variant($payload) => {
                        // Find the module by its variant name (e.g., "Time")
                        if let Some(Module::$mod_variant { service, .. }) = $self.inner.get_mut(stringify!($mod_variant)) {
                            return service.handle_event($payload);
                        }
                    }
                ),*
            }
            // If the module wasn't found (e.g., not in config), do nothing.
            iced::Task::none()
        }
    };
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
        if let MouseEvent::Workspace(id) = event
            && let Some(Module::Niri { service, .. }) =
                self.inner.get_mut("Niri")
        {
            service.hovered_workspace_id = Some(id);
        }
        Task::none()
    }

    pub fn handle_mouse_exited(&mut self, event: MouseEvent) -> Task<Message> {
        if let MouseEvent::Workspace(..) = event
            && let Some(Module::Niri { service, .. }) =
                self.inner.get_mut("Niri")
        {
            service.hovered_workspace_id = None;
        }
        Task::none()
    }

    impl_handle_event_for_modules! {
        self, module_message,
        Tick(time) => Time,
        UpdateBattery(event) => Battery,
        Niri(event) => Niri,
        CavaUpdate(event) => Cava,
        Mpris(event) => Mpris
    }
}

// #[profiling::function]
// pub fn process_modules(
//     config: &mut Config,
//     battery_views: &mut Vec<BatteryView>,
//     time_views: &mut Vec<TimeView>,
//     cava_views: &mut Vec<CavaView>,
//     mpris_views: &mut Vec<MprisView>,
//     niri_views: &mut Vec<NiriView>,
//     label_views: &mut Vec<LabelView>,
// ) {
//     battery_views.clear();
//     time_views.clear();
//     cava_views.clear();
//     mpris_views.clear();
//     niri_views.clear();
//     label_views.clear();
//
//     let mut idx = 0;
//
//     for module in config.start.modules.drain(..) {
//         handle_module(
//             module,
//             BarPosition {
//                 idx,
//                 align: BarAlignment::Start,
//             },
//             battery_views,
//             time_views,
//             cava_views,
//             mpris_views,
//             niri_views,
//             label_views,
//         );
//         idx += 1;
//     }
//
//     for module in config.middle.modules.drain(..) {
//         handle_module(
//             module,
//             BarPosition {
//                 idx,
//                 align: BarAlignment::Middle,
//             },
//             battery_views,
//             time_views,
//             cava_views,
//             mpris_views,
//             niri_views,
//             label_views,
//         );
//         idx += 1;
//     }
//
//     for module in config.end.modules.drain(..) {
//         handle_module(
//             module,
//             BarPosition {
//                 idx,
//                 align: BarAlignment::End,
//             },
//             battery_views,
//             time_views,
//             cava_views,
//             mpris_views,
//             niri_views,
//             label_views,
//         );
//         idx += 1;
//     }
// }

// #[allow(clippy::too_many_arguments)]
// #[profiling::function]
// pub fn handle_module(
//     module: Module,
//     position: BarPosition,
//     battery_views: &mut Vec<BatteryView>,
//     time_views: &mut Vec<TimeView>,
//     cava_views: &mut Vec<CavaView>,
//     mpris_views: &mut Vec<MprisView>,
//     niri_views: &mut Vec<NiriView>,
//     label_views: &mut Vec<LabelView>,
// ) {
//     match module {
//         Module::Battery(config) => {
//             battery_views.push(BatteryView::new(config, position));
//         }
//         Module::Time(config) => {
//             time_views.push(TimeView::new(config, position));
//         }
//         Module::Cava(config) => {
//             cava_views.push(CavaView::new(config, position));
//         }
//         Module::Mpris(config) => {
//             mpris_views.push(MprisView::new(config, position));
//         }
//         Module::Niri(config) => {
//             niri_views.push(NiriView::new(config, position));
//         }
//         Module::Label(config) => {
//             label_views.push(LabelView::new(config, position));
//         }
//     }
// }

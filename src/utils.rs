use tracing_subscriber::EnvFilter;

use crate::{
    config::{Config, Module},
    views::{
        BarPosition, battery::BatteryView, cava::CavaView, mpris::MprisView,
        niri::NiriView, time::TimeView,
    },
};

pub fn handle_module(
    module: Module,
    position: BarPosition,
    battery_views: &mut Vec<BatteryView>,
    time_views: &mut Vec<TimeView>,
    cava_views: &mut Vec<CavaView>,
    mpris_views: &mut Vec<MprisView>,
    niri_views: &mut Vec<NiriView>,
) {
    match module {
        Module::Battery(config) => {
            battery_views.push(BatteryView::new(config, position));
        }
        Module::Time(config) => {
            time_views.push(TimeView::new(config, position));
        }
        Module::Cava(config) => {
            cava_views.push(CavaView::new(config, position));
        }
        Module::Mpris(config) => {
            mpris_views.push(MprisView::new(config, position));
        }
        Module::Niri(config) => {
            niri_views.push(NiriView::new(config, position));
        }
        Module::Label(_) => {}
    }
}

pub fn process_modules(
    config: &mut Config,
    battery_views: &mut Vec<BatteryView>,
    time_views: &mut Vec<TimeView>,
    cava_views: &mut Vec<CavaView>,
    mpris_views: &mut Vec<MprisView>,
    niri_views: &mut Vec<NiriView>,
) {
    battery_views.clear();
    time_views.clear();
    cava_views.clear();
    mpris_views.clear();
    niri_views.clear();

    for module in config.start.modules.drain(..) {
        handle_module(
            module,
            BarPosition::Start,
            battery_views,
            time_views,
            cava_views,
            mpris_views,
            niri_views,
        );
    }

    for module in config.middle.modules.drain(..) {
        handle_module(
            module,
            BarPosition::Middle,
            battery_views,
            time_views,
            cava_views,
            mpris_views,
            niri_views,
        );
    }

    for module in config.end.modules.drain(..) {
        handle_module(
            module,
            BarPosition::Middle,
            battery_views,
            time_views,
            cava_views,
            mpris_views,
            niri_views,
        );
    }
}

pub fn init_tracing() {
    let default_level = if cfg!(debug_assertions) {
        "debug"
    } else {
        "info"
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .init();
}

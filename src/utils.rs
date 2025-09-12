use crate::{
    config::{Config, Module},
    views::{
        BarAlignment, BarPosition, battery::BatteryView, cava::CavaView, label::LabelView,
        mpris::MprisView, niri::NiriView, time::TimeView,
    },
};
use tracing_subscriber::EnvFilter;

pub fn handle_module(
    module: Module,
    position: BarPosition,
    battery_views: &mut Vec<BatteryView>,
    time_views: &mut Vec<TimeView>,
    cava_views: &mut Vec<CavaView>,
    mpris_views: &mut Vec<MprisView>,
    niri_views: &mut Vec<NiriView>,
    label_views: &mut Vec<LabelView>,
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
        Module::Label(config) => {
            label_views.push(LabelView::new(config, position));
        }
    }
}

pub fn process_modules(
    config: &mut Config,
    battery_views: &mut Vec<BatteryView>,
    time_views: &mut Vec<TimeView>,
    cava_views: &mut Vec<CavaView>,
    mpris_views: &mut Vec<MprisView>,
    niri_views: &mut Vec<NiriView>,
    label_views: &mut Vec<LabelView>,
) {
    battery_views.clear();
    time_views.clear();
    cava_views.clear();
    mpris_views.clear();
    niri_views.clear();
    label_views.clear();

    let mut idx = 0;

    for module in config.start.modules.drain(..) {
        handle_module(
            module,
            BarPosition {
                idx,
                align: BarAlignment::Start,
            },
            battery_views,
            time_views,
            cava_views,
            mpris_views,
            niri_views,
            label_views,
        );
        idx += 1;
    }

    for module in config.middle.modules.drain(..) {
        handle_module(
            module,
            BarPosition {
                idx,
                align: BarAlignment::Middle,
            },
            battery_views,
            time_views,
            cava_views,
            mpris_views,
            niri_views,
            label_views,
        );
        idx += 1;
    }

    for module in config.end.modules.drain(..) {
        handle_module(
            module,
            BarPosition {
                idx,
                align: BarAlignment::End,
            },
            battery_views,
            time_views,
            cava_views,
            mpris_views,
            niri_views,
            label_views,
        );
        idx += 1;
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
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::new(
            "%H:%M:%S".to_string(),
        ))
        .with_line_number(true)
        .init();
}

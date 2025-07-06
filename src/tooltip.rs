use iced::{Point, Size};
use lilt::{Animated, Easing};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Tooltip {
    pub content: Option<String>,
    pub animating: Animated<bool, Instant>,
    pub state: TooltipState,
    pub position: Option<Point>,
    pub size: Option<Size>,
}

impl Default for Tooltip {
    fn default() -> Self {
        Self {
            content: None,
            animating: Animated::new(false)
                .duration(175.0)
                .easing(Easing::EaseInOut)
                .delay(30.0),
            state: TooltipState::Hidden,
            position: None,
            size: None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipState {
    Hidden,
    Measuring {
        content_measured: bool,
        position_measured: bool,
        retries: u8,
    },
    AnimatingIn,
    // FullyVisible,
    AnimatingOut,
}

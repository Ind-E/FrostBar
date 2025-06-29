use iced::Point;
use lilt::{Animated, Easing};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Tooltip {
    pub content: Option<String>,
    pub animating: Animated<bool, Instant>,
    pub state: TooltipState,
    pub abort_handle: Option<iced::task::Handle>,
    pub position: Point,
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
            abort_handle: None,
            position: Point::new(0.0, 0.0),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipState {
    Hidden,
    Measuring,
    Visible,
    Hiding,
}

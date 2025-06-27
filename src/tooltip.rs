use iced::window::Id;
use lilt::{Animated, Easing};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Tooltip {
    pub id: Id,
    pub content: Option<String>,
    pub animating: Animated<bool, Instant>,
    pub state: TooltipState,
}

impl Default for Tooltip {
    fn default() -> Self {
        Self {
            id: Id::unique(),
            content: None,
            animating: Animated::new(false)
                .duration(175.0)
                .easing(Easing::EaseInOut)
                .delay(30.0),
            state: TooltipState::Hidden,
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

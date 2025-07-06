use iced::Point;

#[derive(Debug, Clone)]
pub struct Tooltip {
    pub content: Option<String>,
    pub state: TooltipState,
    pub position: Option<Point>,
}

impl Default for Tooltip {
    fn default() -> Self {
        Self {
            content: None,
            state: TooltipState::Hidden,
            position: None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipState {
    Hidden,
    Measuring(u8),
    Visible,
}

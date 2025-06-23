use iced::window::Id;
use tokio::time::Instant;

use crate::ANIMATION_DURATION;

#[derive(Debug, Clone, Copy)]
pub struct Tooltip<State: TooltipMarkerState> {
    pub id: Id,
    pub state: State,
}

#[derive(Clone, Debug)]
pub struct Hidden;
#[derive(Clone, Debug)]
pub struct AnimatingIn {
    content: String,
    pub start: Instant,
}
#[derive(Clone, Debug)]
pub struct AnimatingOut {
    content: String,
    pub start: Instant,
}
#[derive(Clone, Debug)]
pub struct Visible {
    content: String,
}

impl Tooltip<Hidden> {
    pub fn animate_in(self, content: String) -> Tooltip<AnimatingIn> {
        Tooltip {
            id: self.id,
            state: AnimatingIn {
                content,
                start: Instant::now(),
            },
        }
    }
}

impl Tooltip<AnimatingIn> {
    pub fn to_visible(self) -> Tooltip<Visible> {
        Tooltip {
            id: self.id,
            state: Visible {
                content: self.state.content,
            },
        }
    }

    pub fn animate_out(self) -> Tooltip<AnimatingOut> {
        Tooltip {
            id: self.id,
            state: AnimatingOut {
                content: self.state.content,
                start: self.state.start,
            },
        }
    }
}

impl Tooltip<Visible> {
    pub fn animate_out(self) -> Tooltip<AnimatingOut> {
        Tooltip {
            id: self.id,
            state: AnimatingOut {
                content: self.state.content,
                start: Instant::now(),
            },
        }
    }
}

impl Tooltip<AnimatingOut> {
    pub fn animate_in(self) -> Tooltip<AnimatingIn> {
        Tooltip {
            id: self.id,
            state: AnimatingIn {
                content: self.state.content,
                start: Instant::now(),
            },
        }
    }

    pub fn to_hidden(self) -> Tooltip<Hidden> {
        Tooltip {
            id: self.id,
            state: Hidden {},
        }
    }
}

pub trait TooltipMarkerState {}
impl TooltipMarkerState for Hidden {}
impl TooltipMarkerState for AnimatingIn {}
impl TooltipMarkerState for AnimatingOut {}
impl TooltipMarkerState for Visible {}

#[derive(Clone)]
pub enum TooltipState {
    Hidden(Tooltip<Hidden>),
    AnimatingIn(Tooltip<AnimatingIn>),
    Visible(Tooltip<Visible>),
    AnimatingOut(Tooltip<AnimatingOut>),
}

impl TooltipState {
    pub fn id(&self) -> Id {
        match self {
            TooltipState::Hidden(t) => t.id,
            TooltipState::AnimatingIn(t) => t.id,
            TooltipState::Visible(t) => t.id,
            TooltipState::AnimatingOut(t) => t.id,
        }
    }

    pub fn content(&self) -> &str {
        match self {
            TooltipState::Hidden(..) => "",
            TooltipState::AnimatingIn(t) => &t.state.content,
            TooltipState::Visible(t) => &t.state.content,
            TooltipState::AnimatingOut(t) => &t.state.content,
        }
    }

    pub fn progress(&self) -> f32 {
        match self {
            TooltipState::Hidden(..) => 0.0,
            TooltipState::AnimatingIn(t) => {
                let elapsed_ns: u128 = Instant::now().duration_since(t.state.start).as_nanos();
                let duration_ns = ANIMATION_DURATION.as_nanos();

                let progress = (elapsed_ns as f32 / duration_ns as f32).clamp(0.0, 1.0);

                // let eased_progress = 1.0 - (1.0 - progress).powi(1);
                progress
            }
            TooltipState::Visible(..) => 1.0,
            TooltipState::AnimatingOut(t) => {
                let elapsed_ns: u128 = Instant::now().duration_since(t.state.start).as_nanos();
                let duration_ns = ANIMATION_DURATION.as_nanos();

                let progress = 1.0 - (elapsed_ns as f32 / duration_ns as f32).clamp(0.0, 1.0);
                // let eased_progress = 1.0 - (1.0 - progress).powi(3);
                progress
            }
        }
    }
}

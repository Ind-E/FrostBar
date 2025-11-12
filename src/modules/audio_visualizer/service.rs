use super::pipewire::{meter_tap, pw_monitor};
use crate::{Message, modules::ModuleMsg};
use async_channel::Receiver as AsyncChannel;
use iced::{
    Color, Subscription,
    advanced::subscription::{EventStream, Hasher, Recipe, from_recipe},
    futures::{self, StreamExt as _},
};
use std::{fmt, hash::Hasher as _, sync::Arc, time::Duration};

use super::fft::{Fft, MILLIS_PER_FRAME};

pub struct AudioVisualizerService {
    audio_stream: Arc<AsyncChannel<Vec<f32>>>,
    fft: Fft,
    sample_buffer: Vec<f32>,
    last_sample: Vec<f32>,
    silence_frames: u8,
    animating_gravity: bool,
    pub gradient: Option<Vec<Color>>,
    pub bars: Box<[f32]>,
}

const BAR_COUNT: usize = 12;

#[profiling::all_functions]
impl AudioVisualizerService {
    pub fn new() -> Self {
        pw_monitor::run();

        let audio_stream = meter_tap::audio_sample_stream();
        let fft = Fft::new(meter_tap::current_format(), BAR_COUNT);

        Self {
            audio_stream,
            gradient: None,
            animating_gravity: false,
            sample_buffer: Vec::new(),
            last_sample: Vec::new(),
            silence_frames: 0,
            bars: fft.init_bars(),
            fft,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let audio_sub = from_recipe(AudioStreamRecipe {
            audio_stream: self.audio_stream.clone(),
        })
        .map(|sample| Message::Module(ModuleMsg::AudioSample(sample)));
        if let Some(timer_sub) = self.timer_subscription() {
            Subscription::batch([audio_sub, timer_sub])
        } else {
            audio_sub
        }
    }

    pub fn timer_subscription(&self) -> Option<Subscription<Message>> {
        if self.animating_gravity {
            Some(
                iced::time::every(Duration::from_millis(MILLIS_PER_FRAME))
                    .map(|_| Message::Module(ModuleMsg::AudioVisualizerTimer)),
            )
        } else {
            None
        }
    }

    pub fn update(&mut self, new_sample: Vec<f32>) {
        self.sample_buffer.extend(new_sample);

        self.fft.process(Some(&self.sample_buffer), &mut self.bars);

        self.last_sample.clone_from(&self.sample_buffer);
        self.sample_buffer.clear();
        self.silence_frames = 0;

        self.animating_gravity = true;
    }

    pub fn timer_update(&mut self) {
        if self.silence_frames < 3 && !self.last_sample.is_empty() {
            self.silence_frames += 1;
            self.fft.process(Some(&self.last_sample), &mut self.bars);
            self.animating_gravity = true;
        } else {
            self.fft.process(None, &mut self.bars);

            self.animating_gravity = !self.bars.iter().all(|&val| val <= 0.001);
        }
    }

    pub fn update_gradient(&mut self, gradient: Option<Vec<Color>>) {
        self.gradient = gradient;
    }
}

#[derive(Clone)]
struct AudioStreamRecipe<T> {
    audio_stream: Arc<AsyncChannel<T>>,
}

#[profiling::all_functions]
impl<T> Recipe for AudioStreamRecipe<T>
where
    T: Send + 'static,
{
    type Output = T;

    fn hash(&self, state: &mut Hasher) {
        let ptr = Arc::as_ptr(&self.audio_stream) as usize;
        state.write(&ptr.to_ne_bytes());
    }

    fn stream(
        self: Box<Self>,
        _input: EventStream,
    ) -> futures::stream::BoxStream<'static, T> {
        let receiver = Arc::clone(&self.audio_stream);
        futures::stream::unfold(receiver, |receiver| async move {
            match receiver.recv().await {
                Ok(value) => Some((value, receiver)),
                Err(_) => None,
            }
        })
        .boxed()
    }
}

impl<T> fmt::Debug for AudioStreamRecipe<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioStreamRecipe").finish_non_exhaustive()
    }
}

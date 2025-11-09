use super::pipewire::{meter_tap, pw_monitor};
use crate::{
    Message,
    modules::{ModuleMsg, spectrum::fft::Fft},
};
use async_channel::Receiver as AsyncChannel;
use iced::{
    Color, Subscription,
    advanced::subscription::{EventStream, Hasher, Recipe, from_recipe},
    futures::{self, StreamExt as _},
};
use std::{fmt, hash::Hasher as _, sync::Arc};

pub struct SpectrumService {
    audio_stream: Arc<AsyncChannel<Vec<f32>>>,
    fft: Fft,
    pub gradient: Option<Vec<Color>>,
    pub bars: Box<[f32]>,
}

const BAR_COUNT: usize = 12;

#[profiling::all_functions]
impl SpectrumService {
    pub fn new() -> Self {
        pw_monitor::run();

        let audio_stream = meter_tap::audio_sample_stream();

        let fft = Fft::new(meter_tap::current_format(), BAR_COUNT);

        Self {
            audio_stream,
            gradient: None,
            bars: fft.init_bars(),
            fft,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        from_recipe(AudioStreamRecipe {
            audio_stream: self.audio_stream.clone(),
        })
        .map(|sample| Message::Module(ModuleMsg::AudioSample(sample)))
    }

    pub fn update(&mut self, new_samples: Vec<f32>) {
        self.fft.process(&new_samples, &mut self.bars);
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

//! PipeWire virtual sink integration.

use super::{
    ring_buffer::RingBuffer,
    util::{DEFAULT_SAMPLE_RATE, bytes_per_sample, convert_samples_to_f32},
};
use pipewire as pw;
use pw::{properties::properties, spa};
use spa::pod::Pod;
use std::{
    error::Error,
    io::Cursor,
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};
use tracing::{error, info, warn};

const MONITOR_PREFERRED_SAMPLE_RATE: u32 = DEFAULT_SAMPLE_RATE as u32;
const MONITOR_PREFERRED_CHANNELS: u32 = 2;
const CAPTURE_BUFFER_CAPACITY: usize = 256;
const DESIRED_LATENCY_FRAMES: u32 = 256;

static MONITOR_THREAD: OnceLock<thread::JoinHandle<()>> = OnceLock::new();
static CAPTURE_BUFFER: OnceLock<Arc<CaptureBuffer>> = OnceLock::new();

/// Audio block captured from PipeWire with associated format metadata.
#[derive(Debug, Clone)]
pub struct CapturedAudio {
    pub samples: Vec<f32>,
    pub channels: u32,
    pub sample_rate: u32,
}

pub struct CaptureBuffer {
    inner: Mutex<RingBuffer<CapturedAudio>>,
    available: Condvar,
    dropped_frames: AtomicU64,
}

#[profiling::all_functions]
impl CaptureBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(RingBuffer::with_capacity(capacity)),
            available: Condvar::new(),
            dropped_frames: AtomicU64::new(0),
        }
    }

    pub fn try_push(&self, frame: CapturedAudio) {
        match self.inner.try_lock() {
            Ok(mut guard) => {
                if guard.push(frame).is_some() {
                    self.dropped_frames.fetch_add(1, Ordering::Relaxed);
                }
                self.available.notify_one();
            }
            Err(_) => {
                self.dropped_frames.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    #[profiling::skip]
    pub fn pop_wait_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<CapturedAudio>, ()> {
        let mut guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(err) => {
                error!("[virtual-sink] capture buffer lock poisoned: {err}");
                err.into_inner()
            }
        };

        if let Some(frame) = guard.pop() {
            return Ok(Some(frame));
        }

        if timeout.is_zero() {
            return Ok(None);
        }

        loop {
            let (new_guard, wait_result) = match self
                .available
                .wait_timeout(guard, timeout)
            {
                Ok(outcome) => outcome,
                Err(err) => {
                    error!("[virtual-sink] capture buffer wait failed: {err}");
                    let _ = err.into_inner();
                    return Err(());
                }
            };
            guard = new_guard;

            if let Some(frame) = guard.pop() {
                return Ok(Some(frame));
            }

            if wait_result.timed_out() {
                return Ok(None);
            }
        }
    }

    #[cfg(test)]
    fn capacity(&self) -> usize {
        self.inner.lock().map(|guard| guard.capacity()).unwrap_or(0)
    }

    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames.load(Ordering::Relaxed)
    }
}

/// Spawn the monitor in a background thread.
#[profiling::function]
pub fn run() {
    ensure_capture_buffer();

    if MONITOR_THREAD.get().is_some() {
        return;
    }

    match thread::Builder::new()
        .name("frostbar-pw-monitor".into())
        .spawn(|| {
            if let Err(err) = run_monitor_source() {
                error!(target: "pw_monitor", "stopped: {err}");
            }
        }) {
        Ok(handle) => {
            if MONITOR_THREAD.set(handle).is_err() {
                // Another caller raced us; the thread will keep running but we can drop our handle.
            }
        }
        Err(err) => {
            error!(target: "pw_monitor", "failed to start PipeWire thread: {err}")
        }
    }
}

/// Accessor for the shared ring buffer that stores captured audio frames.
pub fn capture_buffer_handle() -> Arc<CaptureBuffer> {
    ensure_capture_buffer().clone()
}

/// Cached parameters describing the negotiated stream format.
struct MonitorState {
    frame_bytes: usize,
    channels: u32,
    sample_rate: u32,
    format: spa::param::audio::AudioFormat,
}

impl MonitorState {
    fn new(channels: u32, sample_rate: u32) -> Self {
        let default_format = spa::param::audio::AudioFormat::F32LE;
        let sample_bytes = bytes_per_sample(default_format)
            .unwrap_or(std::mem::size_of::<f32>());
        let frame_bytes = channels.max(1) as usize * sample_bytes;
        Self {
            frame_bytes,
            channels,
            sample_rate,
            format: default_format,
        }
    }

    fn update_from_info(&mut self, info: &spa::param::audio::AudioInfoRaw) {
        self.channels = info.channels().max(1);
        self.sample_rate = info.rate();
        self.format = info.format();
        if let Some(sample_bytes) = bytes_per_sample(self.format) {
            self.frame_bytes = self.channels as usize * sample_bytes;
        } else {
            warn!(
                target: "pw_monitor",
                "unsupported audio format {:?}; falling back to recorded frame size",
                self.format
            );
        }
        info!(
            target: "pw_monitor",
            "negotiated format: {:?}, rate {} Hz, channels {}",
            info.format(),
            self.sample_rate,
            self.channels
        );
    }
}

/// PipeWire main loop body that registers and services the virtual sink.
#[profiling::function]
fn run_monitor_source() -> Result<(), Box<dyn Error + Send + Sync>> {
    pw::init();

    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;

    let mut props = properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_ROLE => "Music",
        *pw::keys::APP_NAME => "FrostBar",
        *pw::keys::NODE_LATENCY => format!("{}/{}", DESIRED_LATENCY_FRAMES, DEFAULT_SAMPLE_RATE),
        *pw::keys::NODE_PASSIVE => "true",
    };

    props.insert(*pw::keys::STREAM_CAPTURE_SINK, "true");

    let stream = pw::stream::StreamBox::new(&core, "FrostBar Capture", props)?;

    let audio_state = MonitorState::new(
        MONITOR_PREFERRED_CHANNELS,
        MONITOR_PREFERRED_SAMPLE_RATE,
    );
    let capture_buffer = capture_buffer_handle();

    let _listener = stream
        .add_local_listener_with_user_data(audio_state)
        .state_changed(|_, _, previous, current| {
            info!(target: "pw_monitor", "state {previous:?} -> {current:?}");
        })
        .param_changed(|_, state, id, param| {
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }

            if let Some(pod) = param {
                let mut info = spa::param::audio::AudioInfoRaw::new();
                if info.parse(pod).is_ok() {
                    state.update_from_info(&info);
                }
            }
        })
        .process(move |stream, state| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                warn!(target: "pw_monitor", "no buffer available to dequeue");
                return;
            };

            for data in buffer.datas_mut() {
                let used = {
                    let chunk = data.chunk();
                    chunk.size() as usize
                };

                if used == 0 {
                    continue;
                }

                let mut captured = None;
                if let Some(slice) = data.data() {
                    let len = used.min(slice.len());
                    captured =
                        convert_samples_to_f32(&slice[..len], state.format);
                }

                if let Some(samples) = captured {
                    capture_buffer.try_push(CapturedAudio {
                        samples,
                        channels: state.channels,
                        sample_rate: state.sample_rate,
                    });
                }

                let chunk_mut = data.chunk_mut();
                *chunk_mut.offset_mut() = 0;
                *chunk_mut.size_mut() = used as u32;
                *chunk_mut.stride_mut() = state.frame_bytes as i32;
            }
            drop(buffer);
        })
        .register()?;

    let format_bytes = build_format_pod(
        MONITOR_PREFERRED_CHANNELS,
        MONITOR_PREFERRED_SAMPLE_RATE,
    )?;
    let mut params =
        [Pod::from_bytes(&format_bytes).ok_or(pw::Error::CreationFailed)?];

    stream.connect(
        spa::utils::Direction::Input,
        None,
        pw::stream::StreamFlags::AUTOCONNECT
            | pw::stream::StreamFlags::MAP_BUFFERS
            | pw::stream::StreamFlags::RT_PROCESS,
        &mut params,
    )?;

    info!(target: "pw_monitor", "PipeWire monitor active");
    mainloop.run();
    info!(target: "pw_monitor", "main loop exited");

    Ok(())
}

/// Describe the desired raw audio format as a SPA pod for negotiation.
fn build_format_pod(
    channels: u32,
    rate: u32,
) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    let mut info = spa::param::audio::AudioInfoRaw::new();
    info.set_format(spa::param::audio::AudioFormat::F32LE);
    info.set_rate(rate);
    info.set_channels(channels);

    let (cursor, _) = pw::spa::pod::serialize::PodSerializer::serialize(
        Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(pw::spa::pod::Object {
            type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
            id: spa::param::ParamType::EnumFormat.as_raw(),
            properties: info.into(),
        }),
    )?;

    Ok(cursor.into_inner())
}

fn ensure_capture_buffer() -> &'static Arc<CaptureBuffer> {
    CAPTURE_BUFFER
        .get_or_init(|| Arc::new(CaptureBuffer::new(CAPTURE_BUFFER_CAPACITY)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_buffer_is_singleton_with_expected_capacity() {
        let first = ensure_capture_buffer();
        let second = ensure_capture_buffer();
        assert!(Arc::ptr_eq(first, second));

        assert_eq!(first.capacity(), CAPTURE_BUFFER_CAPACITY);
    }

    #[test]
    fn virtual_sink_state_defaults_match_requested_configuration() {
        let state = MonitorState::new(2, DEFAULT_SAMPLE_RATE as u32);
        assert_eq!(state.channels, 2);
        assert_eq!(state.sample_rate, DEFAULT_SAMPLE_RATE as u32);
        assert_eq!(state.format, spa::param::audio::AudioFormat::F32LE);
        assert_eq!(
            state.frame_bytes,
            2 * bytes_per_sample(state.format).unwrap()
        );
    }

    #[test]
    fn update_from_info_clamps_channels_and_updates_frame_bytes() {
        let mut state = MonitorState::new(4, 96_000);
        let mut info = spa::param::audio::AudioInfoRaw::new();
        info.set_channels(0); // PipeWire may report 0 before negotiation; we clamp to at least 1.
        info.set_rate(44_100);
        info.set_format(spa::param::audio::AudioFormat::S16LE);

        state.update_from_info(&info);

        assert_eq!(
            state.channels, 1,
            "channels should be clamped to at least 1"
        );
        assert_eq!(state.sample_rate, 44_100);
        assert_eq!(state.format, spa::param::audio::AudioFormat::S16LE);
        assert_eq!(state.frame_bytes, bytes_per_sample(state.format).unwrap());
    }
}

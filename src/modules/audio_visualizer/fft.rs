use super::pipewire::meter_tap::MeterFormat;
use realfft::{RealFftPlanner, RealToComplex, num_complex::Complex};
use std::{f32::consts::PI, sync::Arc};

const LOW_CUT_OFF: u32 = 50;
const HIGH_CUT_OFF: u32 = 10000;
const BASS_CUT_OFF_HZ: f32 = 100.0;
const NOISE_REDUCTION: f32 = 0.77;
const FRAME_RATE: f32 = 60.0;
pub const MILLIS_PER_FRAME: u64 = (1000.0 / FRAME_RATE) as u64;

#[allow(clippy::struct_field_names)]
pub struct Fft {
    channels: usize,
    bars: usize,

    fft_plan: Arc<dyn RealToComplex<f32>>,
    fft_bass_plan: Arc<dyn RealToComplex<f32>>,

    fft_input: Vec<f32>,
    fft_bass_input: Vec<f32>,
    fft_output: Vec<Complex<f32>>,
    fft_bass_output: Vec<Complex<f32>>,

    hann_window: Vec<f32>,
    hann_bass_window: Vec<f32>,

    bar_cutoff_indices: Vec<(usize, usize)>,
    bass_cutoff_bar: usize,
    eq: Vec<f32>,

    input_buffer: Vec<f32>,
    memory: Box<[f32]>,
    peak: Box<[f32]>,
    fall: Box<[f32]>,
    prev_out: Box<[f32]>,

    sens: f32,
    sens_init: bool,

    last_sample_len: usize,
}

#[profiling::all_functions]
impl Fft {
    pub fn new(format: MeterFormat, bars: usize) -> Self {
        let sample_rate = format.sample_rate as u32;
        let channels = format.channels;

        let mut fft_buffer_size = 512;
        if sample_rate > 8125 {
            fft_buffer_size *= 2;
        }
        if sample_rate > 16250 {
            fft_buffer_size *= 2;
        }
        if sample_rate > 32500 {
            fft_buffer_size *= 2;
        }
        if sample_rate > 75000 {
            fft_buffer_size *= 2;
        }
        if sample_rate > 150000 {
            fft_buffer_size *= 2;
        }
        if sample_rate > 300000 {
            fft_buffer_size *= 2;
        }
        let fft_bass_buffer_size = fft_buffer_size * 2;
        let mut fft_planner = RealFftPlanner::<f32>::new();
        let fft_plan = fft_planner.plan_fft_forward(fft_buffer_size);
        let fft_bass_plan = fft_planner.plan_fft_forward(fft_bass_buffer_size);
        let fft_output_len = fft_plan.complex_len();
        let fft_bass_output_len = fft_bass_plan.complex_len();
        let hann_window: Vec<f32> = (0..fft_buffer_size)
            .map(|i| {
                0.5 * (1.0
                    - (2.0 * PI * i as f32 / (fft_buffer_size - 1) as f32)
                        .cos())
            })
            .collect();
        let hann_bass_window: Vec<f32> = (0..fft_bass_buffer_size)
            .map(|i| {
                0.5 * (1.0
                    - (2.0 * PI * i as f32 / (fft_bass_buffer_size - 1) as f32)
                        .cos())
            })
            .collect();
        let mut cut_off_frequencies = vec![0.0; bars + 1];
        let mut bar_cutoff_indices = vec![(0, 0); bars];
        let mut eq = vec![0.0; bars];
        let frequency_constant = (LOW_CUT_OFF as f32 / HIGH_CUT_OFF as f32)
            .log10()
            / (1.0 / (bars as f32 + 1.0) - 1.0);
        for (n, freq) in
            cut_off_frequencies.iter_mut().enumerate().take(bars + 1)
        {
            let bar_dist_coeff = -frequency_constant
                + ((n as f32 + 1.0) / (bars as f32 + 1.0) * frequency_constant);
            *freq = HIGH_CUT_OFF as f32 * 10.0f32.powf(bar_dist_coeff);
        }
        let mut bass_cutoff_bar = 0;
        let mut lower_cutoff_indices = vec![0; bars + 1];
        for n in 0..=bars {
            let freq = cut_off_frequencies[n];
            if freq < BASS_CUT_OFF_HZ {
                lower_cutoff_indices[n] = (freq
                    / (sample_rate as f32 / fft_bass_buffer_size as f32))
                    as usize;
                if n < bars {
                    bass_cutoff_bar = n + 1;
                }
            } else {
                lower_cutoff_indices[n] = (freq
                    / (sample_rate as f32 / fft_buffer_size as f32))
                    as usize;
            }
        }
        for n in 0..bars {
            let lower_index = lower_cutoff_indices[n].max(3);
            let mut upper_index = lower_cutoff_indices[n + 1];
            if lower_index >= upper_index {
                upper_index = lower_index + 1;
            }
            let max_index = if n < bass_cutoff_bar {
                fft_bass_output_len
            } else {
                fft_output_len
            };
            bar_cutoff_indices[n] = (
                lower_index.clamp(0, max_index),
                (upper_index - 1).clamp(0, max_index),
            );
            let norm_factor = if n < bass_cutoff_bar {
                (fft_bass_buffer_size as f32).log2()
            } else {
                (fft_buffer_size as f32).log2()
            };
            let bar_width =
                (bar_cutoff_indices[n].1 - bar_cutoff_indices[n].0 + 1) as f32;
            eq[n] = cut_off_frequencies[n + 1].powf(1.0) * 2.0f32.powi(-28)
                / norm_factor
                / bar_width.max(1.0);
        }

        Self {
            channels,
            bars,
            fft_plan,
            fft_bass_plan,
            fft_input: vec![0.0; fft_buffer_size],
            fft_bass_input: vec![0.0; fft_bass_buffer_size],
            fft_output: vec![Complex::new(0.0, 0.0); fft_output_len],
            fft_bass_output: vec![Complex::new(0.0, 0.0); fft_bass_output_len],
            hann_window,
            hann_bass_window,
            bar_cutoff_indices,
            bass_cutoff_bar,
            eq,
            input_buffer: vec![0.0; fft_bass_buffer_size * channels],
            memory: vec![0.0; bars * channels].into_boxed_slice(),
            peak: vec![0.0; bars * channels].into_boxed_slice(),
            fall: vec![0.0; bars * channels].into_boxed_slice(),
            prev_out: vec![0.0; bars * channels].into_boxed_slice(),

            sens: 1.0,
            sens_init: true,
            last_sample_len: 0,
        }
    }

    pub fn init_bars(&self) -> Box<[f32]> {
        vec![0.0; self.bars * self.channels].into_boxed_slice()
    }

    pub fn process(
        &mut self,
        new_sample: Option<&[f32]>,
        buffer: &mut Box<[f32]>,
    ) {
        let silence = if let Some(new_sample) = new_sample {
            let new_sample_len = new_sample.len();
            self.last_sample_len = new_sample_len;

            if new_sample_len > 0 {
                let buffer_len = self.input_buffer.len();
                let shift_amount = new_sample_len.min(buffer_len);
                if buffer_len > shift_amount {
                    self.input_buffer.copy_within(
                        0..buffer_len - shift_amount,
                        shift_amount,
                    );
                }
                let fill_range = 0..shift_amount;
                for (dest_idx, sample) in
                    fill_range.zip(new_sample.iter().rev())
                {
                    self.input_buffer[dest_idx] = *sample;
                }
            }

            new_sample.iter().all(|&s| s == 0.0)
        } else {
            let buffer_len = self.input_buffer.len();
            let shift_amount = self.last_sample_len.min(buffer_len);
            if buffer_len > shift_amount {
                self.input_buffer
                    .copy_within(0..buffer_len - shift_amount, shift_amount);
            }
            self.input_buffer[..shift_amount].fill(0.0);
            true
        };

        let mut overshoot = false;

        for ch in 0..self.channels {
            for i in 0..self.fft_bass_input.len() {
                self.fft_bass_input[i] = self.input_buffer
                    [i * self.channels + ch]
                    * self.hann_bass_window[i];
            }
            for i in 0..self.fft_input.len() {
                self.fft_input[i] = self.input_buffer[i * self.channels + ch]
                    * self.hann_window[i];
            }

            self.fft_bass_plan
                .process(&mut self.fft_bass_input, &mut self.fft_bass_output)
                .unwrap();
            self.fft_plan
                .process(&mut self.fft_input, &mut self.fft_output)
                .unwrap();

            for n in 0..self.bars {
                let (start, end) = self.bar_cutoff_indices[n];
                let mut temp_val = 0.0;

                if n < self.bass_cutoff_bar {
                    if start <= end && end < self.fft_bass_output.len() {
                        for i in start..=end {
                            temp_val += self.fft_bass_output[i].norm();
                        }
                    }
                } else if start <= end && end < self.fft_output.len() {
                    for i in start..=end {
                        temp_val += self.fft_output[i].norm();
                    }
                }

                temp_val *= self.eq[n];

                temp_val *= self.sens;

                let out_idx = n + ch * self.bars;
                let mut current_bar_val = temp_val;

                if current_bar_val < self.prev_out[out_idx] {
                    current_bar_val = self.peak[out_idx]
                        * (1.0 - (self.fall[out_idx] * self.fall[out_idx]));
                    if current_bar_val < 0.0 {
                        current_bar_val = 0.0;
                    }
                    self.fall[out_idx] += 0.028;
                } else {
                    self.peak[out_idx] = current_bar_val;
                    self.fall[out_idx] = 0.0;
                }
                self.prev_out[out_idx] = current_bar_val;
                current_bar_val += self.memory[out_idx] * NOISE_REDUCTION;
                self.memory[out_idx] = current_bar_val;

                if current_bar_val > 1.0 {
                    overshoot = true;
                }

                buffer[out_idx] = current_bar_val.clamp(0.0, 1.0);
            }
        }

        if overshoot {
            self.sens *= 0.98;
            self.sens_init = false;
        } else if !silence {
            self.sens *= 1.001;
            if self.sens_init {
                self.sens *= 1.1;
            }
        }
    }
}

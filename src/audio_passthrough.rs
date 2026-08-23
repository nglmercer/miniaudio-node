//! Real-time Audio Passthrough Module
//! Provides low-latency audio loopback from input device to output device

use crate::input::AudioLevels;
use arc_swap::ArcSwapOption;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{Error, Result, Status};
use napi_derive::napi;
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use rodio::cpal;
use rodio::cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rodio::cpal::Sample;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Callback type for audio level updates
type OnLevelsCallback = Box<dyn Fn(AudioLevels) + Send + Sync>;

const DEFAULT_SAMPLE_RATE: u32 = 44100;
const DEFAULT_CHANNELS: u16 = 1;
const DEFAULT_LATENCY_MS: u32 = 20;
const DEVICE_ID_SEPARATOR: char = ':';
const I16_MAX_F32: f32 = 32768.0;
const I8_MAX_F32: f32 = 128.0;
const IO_CHUNK_SIZE: usize = 4096;

/// Stateful converter used by the passthrough input callback. Input and
/// output devices are allowed to have different formats; source frames are
/// buffered so fractional resampling does not restart at every callback.
struct PassthroughConverter {
    source_rate: u32,
    target_rate: u32,
    source_channels: u16,
    target_channels: u16,
    source_buffer: Vec<f32>,
    next_source_frame: f64,
}

impl PassthroughConverter {
    fn new(source_rate: u32, source_channels: u16, target_rate: u32, target_channels: u16) -> Self {
        Self {
            source_rate,
            target_rate,
            source_channels,
            target_channels,
            source_buffer: Vec::with_capacity(IO_CHUNK_SIZE * source_channels.max(1) as usize * 2),
            next_source_frame: 0.0,
        }
    }

    fn convert_into(
        &mut self,
        samples: &[f32],
        producer: &mut HeapProd<f32>,
        output_scratch: &mut [f32; IO_CHUNK_SIZE],
    ) {
        let source_channels = self.source_channels as usize;
        if self.source_rate == 0
            || self.target_rate == 0
            || source_channels == 0
            || self.target_channels == 0
        {
            return;
        }

        let complete_samples = samples.len() / source_channels * source_channels;
        self.source_buffer
            .extend_from_slice(&samples[..complete_samples]);

        let source_frames = self.source_buffer.len() / source_channels;
        let source_step = self.source_rate as f64 / self.target_rate as f64;
        let target_channels = self.target_channels as usize;
        let mut output_len = 0;

        // Keep one look-ahead frame for interpolation. It is retained across
        // callbacks so both upsampling and downsampling keep their phase.
        while self.next_source_frame + 1.0 < source_frames as f64 {
            let source_index = self.next_source_frame.floor() as usize;
            let fraction = (self.next_source_frame - source_index as f64) as f32;
            let next_index = (source_index + 1).min(source_frames - 1);
            for target_channel in 0..target_channels {
                output_scratch[output_len] =
                    self.converted_sample(source_index, next_index, fraction, target_channel);
                output_len += 1;
                if output_len == output_scratch.len() {
                    let _ = producer.push_slice(output_scratch);
                    output_len = 0;
                }
            }
            self.next_source_frame += source_step;
        }

        if output_len > 0 {
            let _ = producer.push_slice(&output_scratch[..output_len]);
        }

        // Discard source frames that can no longer be used as interpolation
        // neighbours and keep the fractional position relative to the buffer.
        let discard_frames = (self.next_source_frame.floor() as usize).saturating_sub(1);
        if discard_frames > 0 {
            let discard_samples = discard_frames * source_channels;
            self.source_buffer.drain(..discard_samples);
            self.next_source_frame -= discard_frames as f64;
        }
    }

    fn interpolated_sample(
        &self,
        frame_index: usize,
        next_frame_index: usize,
        fraction: f32,
        channel: usize,
    ) -> f32 {
        let channels = self.source_channels as usize;
        let first = self.source_buffer[frame_index * channels + channel];
        let second = self.source_buffer[next_frame_index * channels + channel];
        first + (second - first) * fraction
    }

    fn converted_sample(
        &self,
        frame_index: usize,
        next_frame_index: usize,
        fraction: f32,
        target_channel: usize,
    ) -> f32 {
        let source_channels = self.source_channels as usize;
        let target_channels = self.target_channels as usize;
        if source_channels == target_channels {
            return self.interpolated_sample(
                frame_index,
                next_frame_index,
                fraction,
                target_channel,
            );
        }

        let source_average = || {
            (0..source_channels)
                .map(|channel| {
                    self.interpolated_sample(frame_index, next_frame_index, fraction, channel)
                })
                .sum::<f32>()
                / source_channels as f32
        };

        if target_channels == 1 {
            return source_average();
        }
        if source_channels == 1 {
            return self.interpolated_sample(frame_index, next_frame_index, fraction, 0);
        }
        if target_channels < source_channels {
            let mut sum = 0.0;
            let mut count = 0usize;
            for source_channel in (target_channel..source_channels).step_by(target_channels) {
                sum += self.interpolated_sample(
                    frame_index,
                    next_frame_index,
                    fraction,
                    source_channel,
                );
                count += 1;
            }
            return if count == 0 {
                source_average()
            } else {
                sum / count as f32
            };
        }
        if target_channel < source_channels {
            self.interpolated_sample(frame_index, next_frame_index, fraction, target_channel)
        } else {
            source_average()
        }
    }
}

/// Real-time audio passthrough (loopback) from input to output
/// Uses a ring buffer to transfer audio data between input and output streams
/// with minimal latency
#[napi]
pub struct AudioPassthrough {
    // Streams
    input_stream: Option<cpal::Stream>,
    output_stream: Option<cpal::Stream>,

    // State
    is_running: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,

    // Audio levels
    last_peak: Arc<AtomicU64>,
    last_rms: Arc<AtomicU64>,

    // Callbacks
    on_levels_callback: Arc<ArcSwapOption<OnLevelsCallback>>,
}

impl Default for AudioPassthrough {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioPassthrough {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[napi]
impl AudioPassthrough {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            input_stream: None,
            output_stream: None,
            is_running: Arc::new(AtomicBool::new(false)),
            sample_rate: DEFAULT_SAMPLE_RATE,
            channels: DEFAULT_CHANNELS,
            last_peak: Arc::new(AtomicU64::new(0.0f64.to_bits())),
            last_rms: Arc::new(AtomicU64::new(0.0f64.to_bits())),
            on_levels_callback: Arc::new(ArcSwapOption::empty()),
        }
    }

    /// Set callback for audio level updates (peak, RMS)
    #[napi]
    pub fn set_on_levels(&self, callback: ThreadsafeFunction<AudioLevels>) -> Result<()> {
        let cb = Box::new(move |levels: AudioLevels| {
            callback.call(
                Ok::<_, Error>(levels),
                ThreadsafeFunctionCallMode::NonBlocking,
            );
        });

        self.on_levels_callback.store(Some(Arc::new(cb)));
        Ok(())
    }

    /// Start the audio passthrough
    ///
    /// # Arguments
    /// * `input_device_id` - Input device ID (e.g., "Alsa:13") or None for default
    /// * `output_device_id` - Output device ID or None for default
    /// * `latency_ms` - Target latency in milliseconds (default: 20)
    #[napi]
    pub fn start(
        &mut self,
        input_device_id: Option<String>,
        output_device_id: Option<String>,
        latency_ms: Option<u32>,
    ) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err(Error::new(
                Status::GenericFailure,
                "Passthrough is already running",
            ));
        }

        let target_latency = latency_ms.unwrap_or(DEFAULT_LATENCY_MS);

        // Get input device
        let host = cpal::default_host();

        // `None` and an empty ID both mean the host's default input device.
        // Keep this path shared with the explicit device lookup helper so the
        // default-device contract cannot silently diverge.
        let input_device = if input_device_id.as_ref().is_some_and(|s| !s.is_empty()) {
            self.get_input_device(&host, input_device_id.as_deref())?
        } else {
            self.get_input_device(&host, None)?
        };

        // Get output device. Empty IDs follow the same default-device
        // contract as omitted IDs.
        let output_device = if output_device_id.as_ref().is_some_and(|s| !s.is_empty()) {
            self.get_output_device(&host, output_device_id.as_deref())?
        } else {
            self.get_output_device(&host, None)?
        };

        // Get input config
        let input_config = input_device.default_input_config().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to get input config: {}", e),
            )
        })?;

        self.sample_rate = input_config.sample_rate().0;
        self.channels = input_config.channels();

        // Negotiate the output independently. Input and output devices often
        // use different rates, channel counts, and scalar formats.
        let output_config = output_device.default_output_config().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to get output config: {}", e),
            )
        })?;

        // Ring-buffer capacity is expressed in output samples because the
        // output callback consumes the converted stream.
        let samples_per_buffer = (output_config.sample_rate().0 as u64
            * output_config.channels() as u64
            * target_latency as u64
            / 1000) as usize;
        let buffer_size = samples_per_buffer.max(1) * 4;
        let (producer, consumer) = HeapRb::<f32>::new(buffer_size).split();

        // Only the immutable state and callback handles cross into the audio
        // callbacks. Producer/consumer ownership keeps the SPSC ring lock-free.
        let is_running = self.is_running.clone();
        let last_peak = self.last_peak.clone();
        let last_rms = self.last_rms.clone();
        let on_levels = self.on_levels_callback.clone();
        let converter = PassthroughConverter::new(
            input_config.sample_rate().0,
            input_config.channels(),
            output_config.sample_rate().0,
            output_config.channels(),
        );

        // Build input stream
        let input_stream_config: cpal::StreamConfig = input_config.clone().into();

        let err_fn = |err| {
            eprintln!("Input stream error: {}", err);
        };

        // Create input stream. Each callback owns its producer, converter, and
        // fixed scratch arrays; no shared ring-buffer mutex or temporary Vec is
        // used on the input path.
        let input_stream = match input_config.sample_format() {
            cpal::SampleFormat::F32 => {
                let mut producer = producer;
                let mut converter = converter;
                let mut output_scratch = [0.0; IO_CHUNK_SIZE];
                let mut input_scratch = [0.0; IO_CHUNK_SIZE];
                let is_running = is_running.clone();
                let last_peak = last_peak.clone();
                let last_rms = last_rms.clone();
                let on_levels = on_levels.clone();
                input_device.build_input_stream(
                    &input_stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if is_running.load(Ordering::Relaxed) {
                            process_typed_input(
                                data,
                                &mut input_scratch,
                                f32_from_f32,
                                &mut producer,
                                &mut converter,
                                &mut output_scratch,
                                &last_peak,
                                &last_rms,
                                &on_levels,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let mut producer = producer;
                let mut converter = converter;
                let mut output_scratch = [0.0; IO_CHUNK_SIZE];
                let mut input_scratch = [0.0; IO_CHUNK_SIZE];
                let is_running = is_running.clone();
                let last_peak = last_peak.clone();
                let last_rms = last_rms.clone();
                let on_levels = on_levels.clone();
                input_device.build_input_stream(
                    &input_stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if is_running.load(Ordering::Relaxed) {
                            process_typed_input(
                                data,
                                &mut input_scratch,
                                i16_to_f32,
                                &mut producer,
                                &mut converter,
                                &mut output_scratch,
                                &last_peak,
                                &last_rms,
                                &on_levels,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I8 => {
                let mut producer = producer;
                let mut converter = converter;
                let mut output_scratch = [0.0; IO_CHUNK_SIZE];
                let mut input_scratch = [0.0; IO_CHUNK_SIZE];
                let is_running = is_running.clone();
                let last_peak = last_peak.clone();
                let last_rms = last_rms.clone();
                let on_levels = on_levels.clone();
                input_device.build_input_stream(
                    &input_stream_config,
                    move |data: &[i8], _: &cpal::InputCallbackInfo| {
                        if is_running.load(Ordering::Relaxed) {
                            process_typed_input(
                                data,
                                &mut input_scratch,
                                i8_to_f32,
                                &mut producer,
                                &mut converter,
                                &mut output_scratch,
                                &last_peak,
                                &last_rms,
                                &on_levels,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I24 => {
                let mut producer = producer;
                let mut converter = converter;
                let mut output_scratch = [0.0; IO_CHUNK_SIZE];
                let mut input_scratch = [0.0; IO_CHUNK_SIZE];
                let is_running = is_running.clone();
                let last_peak = last_peak.clone();
                let last_rms = last_rms.clone();
                let on_levels = on_levels.clone();
                input_device.build_input_stream(
                    &input_stream_config,
                    move |data: &[cpal::I24], _: &cpal::InputCallbackInfo| {
                        if is_running.load(Ordering::Relaxed) {
                            process_typed_input(
                                data,
                                &mut input_scratch,
                                i24_to_f32,
                                &mut producer,
                                &mut converter,
                                &mut output_scratch,
                                &last_peak,
                                &last_rms,
                                &on_levels,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I32 => {
                let mut producer = producer;
                let mut converter = converter;
                let mut output_scratch = [0.0; IO_CHUNK_SIZE];
                let mut input_scratch = [0.0; IO_CHUNK_SIZE];
                let is_running = is_running.clone();
                let last_peak = last_peak.clone();
                let last_rms = last_rms.clone();
                let on_levels = on_levels.clone();
                input_device.build_input_stream(
                    &input_stream_config,
                    move |data: &[i32], _: &cpal::InputCallbackInfo| {
                        if is_running.load(Ordering::Relaxed) {
                            process_typed_input(
                                data,
                                &mut input_scratch,
                                i32_to_f32,
                                &mut producer,
                                &mut converter,
                                &mut output_scratch,
                                &last_peak,
                                &last_rms,
                                &on_levels,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I64 => {
                let mut producer = producer;
                let mut converter = converter;
                let mut output_scratch = [0.0; IO_CHUNK_SIZE];
                let mut input_scratch = [0.0; IO_CHUNK_SIZE];
                let is_running = is_running.clone();
                let last_peak = last_peak.clone();
                let last_rms = last_rms.clone();
                let on_levels = on_levels.clone();
                input_device.build_input_stream(
                    &input_stream_config,
                    move |data: &[i64], _: &cpal::InputCallbackInfo| {
                        if is_running.load(Ordering::Relaxed) {
                            process_typed_input(
                                data,
                                &mut input_scratch,
                                i64_to_f32,
                                &mut producer,
                                &mut converter,
                                &mut output_scratch,
                                &last_peak,
                                &last_rms,
                                &on_levels,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U8 => {
                let mut producer = producer;
                let mut converter = converter;
                let mut output_scratch = [0.0; IO_CHUNK_SIZE];
                let mut input_scratch = [0.0; IO_CHUNK_SIZE];
                let is_running = is_running.clone();
                let last_peak = last_peak.clone();
                let last_rms = last_rms.clone();
                let on_levels = on_levels.clone();
                input_device.build_input_stream(
                    &input_stream_config,
                    move |data: &[u8], _: &cpal::InputCallbackInfo| {
                        if is_running.load(Ordering::Relaxed) {
                            process_typed_input(
                                data,
                                &mut input_scratch,
                                u8_to_f32,
                                &mut producer,
                                &mut converter,
                                &mut output_scratch,
                                &last_peak,
                                &last_rms,
                                &on_levels,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let mut producer = producer;
                let mut converter = converter;
                let mut output_scratch = [0.0; IO_CHUNK_SIZE];
                let mut input_scratch = [0.0; IO_CHUNK_SIZE];
                let is_running = is_running.clone();
                let last_peak = last_peak.clone();
                let last_rms = last_rms.clone();
                let on_levels = on_levels.clone();
                input_device.build_input_stream(
                    &input_stream_config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if is_running.load(Ordering::Relaxed) {
                            process_typed_input(
                                data,
                                &mut input_scratch,
                                u16_to_f32,
                                &mut producer,
                                &mut converter,
                                &mut output_scratch,
                                &last_peak,
                                &last_rms,
                                &on_levels,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U32 => {
                let mut producer = producer;
                let mut converter = converter;
                let mut output_scratch = [0.0; IO_CHUNK_SIZE];
                let mut input_scratch = [0.0; IO_CHUNK_SIZE];
                let is_running = is_running.clone();
                let last_peak = last_peak.clone();
                let last_rms = last_rms.clone();
                let on_levels = on_levels.clone();
                input_device.build_input_stream(
                    &input_stream_config,
                    move |data: &[u32], _: &cpal::InputCallbackInfo| {
                        if is_running.load(Ordering::Relaxed) {
                            process_typed_input(
                                data,
                                &mut input_scratch,
                                u32_to_f32,
                                &mut producer,
                                &mut converter,
                                &mut output_scratch,
                                &last_peak,
                                &last_rms,
                                &on_levels,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U64 => {
                let mut producer = producer;
                let mut converter = converter;
                let mut output_scratch = [0.0; IO_CHUNK_SIZE];
                let mut input_scratch = [0.0; IO_CHUNK_SIZE];
                let is_running = is_running.clone();
                let last_peak = last_peak.clone();
                let last_rms = last_rms.clone();
                let on_levels = on_levels.clone();
                input_device.build_input_stream(
                    &input_stream_config,
                    move |data: &[u64], _: &cpal::InputCallbackInfo| {
                        if is_running.load(Ordering::Relaxed) {
                            process_typed_input(
                                data,
                                &mut input_scratch,
                                u64_to_f32,
                                &mut producer,
                                &mut converter,
                                &mut output_scratch,
                                &last_peak,
                                &last_rms,
                                &on_levels,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::F64 => {
                let mut producer = producer;
                let mut converter = converter;
                let mut output_scratch = [0.0; IO_CHUNK_SIZE];
                let mut input_scratch = [0.0; IO_CHUNK_SIZE];
                let is_running = is_running.clone();
                let last_peak = last_peak.clone();
                let last_rms = last_rms.clone();
                let on_levels = on_levels.clone();
                input_device.build_input_stream(
                    &input_stream_config,
                    move |data: &[f64], _: &cpal::InputCallbackInfo| {
                        if is_running.load(Ordering::Relaxed) {
                            process_typed_input(
                                data,
                                &mut input_scratch,
                                f64_to_f32,
                                &mut producer,
                                &mut converter,
                                &mut output_scratch,
                                &last_peak,
                                &last_rms,
                                &on_levels,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
            }
            _ => {
                return Err(Error::new(
                    Status::GenericFailure,
                    format!(
                        "Unsupported input sample format: {:?}",
                        input_config.sample_format()
                    ),
                ))
            }
        }
        .map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to build input stream: {}", e),
            )
        })?;

        // Build the output stream with the output device's own configuration.
        // The callback converts normalized f32 samples to the device scalar
        // type instead of assuming that every output is f32.
        let output_stream_config: cpal::StreamConfig = output_config.clone().into();
        let output_stream = match output_config.sample_format() {
            cpal::SampleFormat::F32 => {
                let mut consumer = consumer;
                let mut scratch = [0.0; IO_CHUNK_SIZE];
                output_device.build_output_stream(
                    &output_stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        fill_output(data, &mut consumer, &mut scratch, f32_from_f32);
                    },
                    |err| eprintln!("Output stream error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let mut consumer = consumer;
                let mut scratch = [0.0; IO_CHUNK_SIZE];
                output_device.build_output_stream(
                    &output_stream_config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        fill_output(data, &mut consumer, &mut scratch, f32_to_i16);
                    },
                    |err| eprintln!("Output stream error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::I24 => {
                let mut consumer = consumer;
                let mut scratch = [0.0; IO_CHUNK_SIZE];
                output_device.build_output_stream(
                    &output_stream_config,
                    move |data: &mut [cpal::I24], _: &cpal::OutputCallbackInfo| {
                        fill_output(data, &mut consumer, &mut scratch, f32_to_i24);
                    },
                    |err| eprintln!("Output stream error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::I8 => {
                let mut consumer = consumer;
                let mut scratch = [0.0; IO_CHUNK_SIZE];
                output_device.build_output_stream(
                    &output_stream_config,
                    move |data: &mut [i8], _: &cpal::OutputCallbackInfo| {
                        fill_output(data, &mut consumer, &mut scratch, f32_to_i8);
                    },
                    |err| eprintln!("Output stream error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::U8 => {
                let mut consumer = consumer;
                let mut scratch = [0.0; IO_CHUNK_SIZE];
                output_device.build_output_stream(
                    &output_stream_config,
                    move |data: &mut [u8], _: &cpal::OutputCallbackInfo| {
                        fill_output(data, &mut consumer, &mut scratch, f32_to_u8);
                    },
                    |err| eprintln!("Output stream error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let mut consumer = consumer;
                let mut scratch = [0.0; IO_CHUNK_SIZE];
                output_device.build_output_stream(
                    &output_stream_config,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        fill_output(data, &mut consumer, &mut scratch, f32_to_u16);
                    },
                    |err| eprintln!("Output stream error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::I32 => {
                let mut consumer = consumer;
                let mut scratch = [0.0; IO_CHUNK_SIZE];
                output_device.build_output_stream(
                    &output_stream_config,
                    move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                        fill_output(data, &mut consumer, &mut scratch, f32_to_i32);
                    },
                    |err| eprintln!("Output stream error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::I64 => {
                let mut consumer = consumer;
                let mut scratch = [0.0; IO_CHUNK_SIZE];
                output_device.build_output_stream(
                    &output_stream_config,
                    move |data: &mut [i64], _: &cpal::OutputCallbackInfo| {
                        fill_output(data, &mut consumer, &mut scratch, f32_to_i64);
                    },
                    |err| eprintln!("Output stream error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::U32 => {
                let mut consumer = consumer;
                let mut scratch = [0.0; IO_CHUNK_SIZE];
                output_device.build_output_stream(
                    &output_stream_config,
                    move |data: &mut [u32], _: &cpal::OutputCallbackInfo| {
                        fill_output(data, &mut consumer, &mut scratch, f32_to_u32);
                    },
                    |err| eprintln!("Output stream error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::U64 => {
                let mut consumer = consumer;
                let mut scratch = [0.0; IO_CHUNK_SIZE];
                output_device.build_output_stream(
                    &output_stream_config,
                    move |data: &mut [u64], _: &cpal::OutputCallbackInfo| {
                        fill_output(data, &mut consumer, &mut scratch, f32_to_u64);
                    },
                    |err| eprintln!("Output stream error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::F64 => {
                let mut consumer = consumer;
                let mut scratch = [0.0; IO_CHUNK_SIZE];
                output_device.build_output_stream(
                    &output_stream_config,
                    move |data: &mut [f64], _: &cpal::OutputCallbackInfo| {
                        fill_output(data, &mut consumer, &mut scratch, f32_to_f64);
                    },
                    |err| eprintln!("Output stream error: {}", err),
                    None,
                )
            }
            unsupported => {
                return Err(Error::new(
                    Status::GenericFailure,
                    format!("Unsupported output sample format: {:?}", unsupported),
                ))
            }
        }
        .map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to build output stream: {}", e),
            )
        })?;

        // Start both streams
        input_stream.play().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to start input stream: {}", e),
            )
        })?;

        output_stream.play().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to start output stream: {}", e),
            )
        })?;

        self.input_stream = Some(input_stream);
        self.output_stream = Some(output_stream);
        self.is_running.store(true, Ordering::SeqCst);

        Ok(())
    }

    /// Stop the audio passthrough
    #[napi]
    pub fn stop(&mut self) -> Result<()> {
        if !self.is_running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.is_running.store(false, Ordering::SeqCst);

        // Drop streams
        self.input_stream = None;
        self.output_stream = None;

        Ok(())
    }

    /// Check if passthrough is running
    #[napi]
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Get current audio levels
    #[napi]
    pub fn get_levels(&self) -> AudioLevels {
        AudioLevels {
            peak: f64::from_bits(self.last_peak.load(Ordering::Relaxed)),
            rms: f64::from_bits(self.last_rms.load(Ordering::Relaxed)),
        }
    }

    /// Get the current sample rate
    #[napi]
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the current channel count
    #[napi]
    pub fn get_channels(&self) -> u16 {
        self.channels
    }

    /// Get available input devices
    #[napi]
    pub fn get_input_devices() -> Vec<crate::types::AudioDeviceInfo> {
        crate::input::get_input_devices().unwrap_or_default()
    }

    /// Get available output devices
    #[napi]
    pub fn get_output_devices() -> Result<Vec<crate::types::AudioDeviceInfo>> {
        let mut result = Vec::new();
        for host_id in cpal::available_hosts() {
            let host = match cpal::host_from_id(host_id) {
                Ok(host) => host,
                Err(_) => continue,
            };
            let host_name = format!("{:?}", host_id);
            let default_device = host.default_output_device();
            let devices = match host.output_devices() {
                Ok(devices) => devices,
                Err(_) => continue,
            };

            for (i, device) in devices.enumerate() {
                if let Ok(name) = device.name() {
                    let name_lower = name.to_lowercase();
                    if name_lower.contains("null") || name_lower.contains("discard") {
                        continue;
                    }

                    let is_default = default_device.as_ref().is_some_and(|d| {
                        d.name()
                            .map(|default_name| default_name == name)
                            .unwrap_or(false)
                    });

                    result.push(crate::types::AudioDeviceInfo {
                        id: format!("{}:{}", host_name, i),
                        name,
                        host: host_name.clone(),
                        is_default,
                    });
                }
            }
        }

        Ok(result)
    }

    // Helper to get input device
    fn get_input_device(&self, host: &cpal::Host, device_id: Option<&str>) -> Result<cpal::Device> {
        match device_id {
            Some(id) => {
                if let Some((host_name, index)) = id.split_once(DEVICE_ID_SEPARATOR) {
                    let device_idx = index.parse::<usize>().map_err(|_| {
                        Error::new(Status::InvalidArg, format!("Invalid device index: {}", id))
                    })?;

                    let host_id = cpal::available_hosts()
                        .into_iter()
                        .find(|h| format!("{:?}", h).to_lowercase() == host_name.to_lowercase())
                        .ok_or_else(|| {
                            Error::new(
                                Status::InvalidArg,
                                format!("Host '{}' not found", host_name),
                            )
                        })?;

                    let host = cpal::host_from_id(host_id)
                        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

                    host.input_devices()
                        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                        .nth(device_idx)
                        .ok_or_else(|| {
                            Error::new(
                                Status::InvalidArg,
                                format!("Device at index {} not found", device_idx),
                            )
                        })
                } else {
                    let idx = id.parse::<usize>().map_err(|_| {
                        Error::new(Status::InvalidArg, format!("Invalid device ID: {}", id))
                    })?;
                    host.input_devices()
                        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                        .nth(idx)
                        .ok_or_else(|| Error::new(Status::InvalidArg, "Device not found"))
                }
            }
            None => host
                .default_input_device()
                .ok_or_else(|| Error::new(Status::GenericFailure, "No default input device")),
        }
    }

    // Helper to get output device
    fn get_output_device(
        &self,
        host: &cpal::Host,
        device_id: Option<&str>,
    ) -> Result<cpal::Device> {
        match device_id {
            Some(id) => {
                if let Some((host_name, index)) = id.split_once(DEVICE_ID_SEPARATOR) {
                    let device_idx = index.parse::<usize>().map_err(|_| {
                        Error::new(Status::InvalidArg, format!("Invalid device index: {}", id))
                    })?;

                    let host_id = cpal::available_hosts()
                        .into_iter()
                        .find(|h| format!("{:?}", h).to_lowercase() == host_name.to_lowercase())
                        .ok_or_else(|| {
                            Error::new(
                                Status::InvalidArg,
                                format!("Host '{}' not found", host_name),
                            )
                        })?;

                    let host = cpal::host_from_id(host_id)
                        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

                    host.output_devices()
                        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                        .nth(device_idx)
                        .ok_or_else(|| {
                            Error::new(
                                Status::InvalidArg,
                                format!("Device at index {} not found", device_idx),
                            )
                        })
                } else {
                    let idx = id.parse::<usize>().map_err(|_| {
                        Error::new(Status::InvalidArg, format!("Invalid device ID: {}", id))
                    })?;
                    host.output_devices()
                        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                        .nth(idx)
                        .ok_or_else(|| Error::new(Status::InvalidArg, "Device not found"))
                }
            }
            None => host
                .default_output_device()
                .ok_or_else(|| Error::new(Status::GenericFailure, "No default output device")),
        }
    }
}

/// Process input audio data - calculate levels and push to ring buffer
fn process_input_data(
    data: &[f32],
    producer: &mut HeapProd<f32>,
    converter: &mut PassthroughConverter,
    output_scratch: &mut [f32; IO_CHUNK_SIZE],
    last_peak: &AtomicU64,
    last_rms: &AtomicU64,
    on_levels: &Arc<ArcSwapOption<OnLevelsCallback>>,
) {
    // Calculate peak and RMS
    let mut peak: f32 = 0.0;
    let mut sum_sq: f64 = 0.0;

    for &sample in data {
        let abs_sample = sample.abs();
        if abs_sample > peak {
            peak = abs_sample;
        }
        sum_sq += (sample as f64) * (sample as f64);
    }

    let rms = if !data.is_empty() {
        (sum_sq / data.len() as f64).sqrt() as f32
    } else {
        0.0
    };

    // Update last levels
    last_peak.store((peak as f64).to_bits(), Ordering::Relaxed);
    last_rms.store((rms as f64).to_bits(), Ordering::Relaxed);

    // Emit callback
    if let Some(cb) = on_levels.load().as_ref() {
        cb(AudioLevels {
            peak: peak as f64,
            rms: rms as f64,
        });
    }

    converter.convert_into(data, producer, output_scratch);
}

// The callback's state is deliberately passed as explicit references so the
// producer, converter, scratch buffers, and level handles remain owned by the
// callback thread. This avoids a shared lock on the audio path.
#[allow(clippy::too_many_arguments)]
fn process_typed_input<T: Copy>(
    data: &[T],
    input_scratch: &mut [f32; IO_CHUNK_SIZE],
    convert: fn(T) -> f32,
    producer: &mut HeapProd<f32>,
    converter: &mut PassthroughConverter,
    output_scratch: &mut [f32; IO_CHUNK_SIZE],
    last_peak: &AtomicU64,
    last_rms: &AtomicU64,
    on_levels: &Arc<ArcSwapOption<OnLevelsCallback>>,
) {
    for chunk in data.chunks(IO_CHUNK_SIZE) {
        for (index, sample) in chunk.iter().copied().enumerate() {
            input_scratch[index] = convert(sample);
        }
        process_input_data(
            &input_scratch[..chunk.len()],
            producer,
            converter,
            output_scratch,
            last_peak,
            last_rms,
            on_levels,
        );
    }
}

fn i16_to_f32(sample: i16) -> f32 {
    sample as f32 / I16_MAX_F32
}

fn i8_to_f32(sample: i8) -> f32 {
    sample as f32 / I8_MAX_F32
}

fn i24_to_f32(sample: cpal::I24) -> f32 {
    sample.to_float_sample()
}

fn i32_to_f32(sample: i32) -> f32 {
    sample as f32 / 2_147_483_648.0
}

fn i64_to_f32(sample: i64) -> f32 {
    (sample as f64 / 9_223_372_036_854_775_808.0) as f32
}

fn u8_to_f32(sample: u8) -> f32 {
    (sample as f32 - 128.0) / 128.0
}

fn u16_to_f32(sample: u16) -> f32 {
    (sample as f32 - I16_MAX_F32) / I16_MAX_F32
}

fn u32_to_f32(sample: u32) -> f32 {
    ((sample as f64 - 2_147_483_648.0) / 2_147_483_648.0) as f32
}

fn u64_to_f32(sample: u64) -> f32 {
    ((sample as f64 - 9_223_372_036_854_775_808.0) / 9_223_372_036_854_775_808.0) as f32
}

fn f64_to_f32(sample: f64) -> f32 {
    sample as f32
}

fn f32_from_f32(sample: f32) -> f32 {
    sample
}

fn f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

fn f32_to_i24(sample: f32) -> cpal::I24 {
    cpal::I24::from_sample(sample.clamp(-1.0, 1.0))
}

fn f32_to_i8(sample: f32) -> i8 {
    (sample.clamp(-1.0, 1.0) * i8::MAX as f32) as i8
}

fn f32_to_u8(sample: f32) -> u8 {
    ((sample.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
}

fn f32_to_u16(sample: f32) -> u16 {
    ((sample.clamp(-1.0, 1.0) + 1.0) * 32767.5) as u16
}

fn f32_to_i32(sample: f32) -> i32 {
    (sample.clamp(-1.0, 1.0) * i32::MAX as f32) as i32
}

fn f32_to_i64(sample: f32) -> i64 {
    (sample.clamp(-1.0, 1.0) as f64 * i64::MAX as f64) as i64
}

fn f32_to_u32(sample: f32) -> u32 {
    ((sample.clamp(-1.0, 1.0) + 1.0) * 2_147_483_647.5) as u32
}

fn f32_to_u64(sample: f32) -> u64 {
    ((sample.clamp(-1.0, 1.0) as f64 + 1.0) * 9_223_372_036_854_775_807.5) as u64
}

fn f32_to_f64(sample: f32) -> f64 {
    sample as f64
}

fn fill_output<T: Copy>(
    data: &mut [T],
    consumer: &mut HeapCons<f32>,
    scratch: &mut [f32; IO_CHUNK_SIZE],
    convert: fn(f32) -> T,
) {
    let mut offset = 0;
    while offset < data.len() {
        let count_requested = (data.len() - offset).min(scratch.len());
        let count = consumer.pop_slice(&mut scratch[..count_requested]);
        for (index, sample) in scratch[..count].iter().copied().enumerate() {
            data[offset + index] = convert(sample);
        }
        offset += count;
        if count < count_requested {
            for sample in &mut data[offset..] {
                *sample = convert(0.0);
            }
            break;
        }
    }
}

/// Simple audio passthrough with minimal configuration
///
/// # Arguments
/// * `input_device` - Input device ID (e.g., "Alsa:13") or null for default
/// * `output_device` - Output device ID or null for default
/// * `latency_ms` - Target latency in milliseconds
#[napi]
pub fn start_passthrough(
    input_device: Option<String>,
    output_device: Option<String>,
    latency_ms: Option<u32>,
) -> Result<AudioPassthrough> {
    let mut passthrough = AudioPassthrough::new();
    passthrough.start(input_device, output_device, latency_ms)?;
    Ok(passthrough)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converter_resamples_interleaved_channels_without_cross_talk() {
        let mut converter = PassthroughConverter::new(48_000, 2, 44_100, 2);
        let (mut producer, mut consumer) = HeapRb::<f32>::new(64).split();
        let mut scratch = [0.0; IO_CHUNK_SIZE];
        converter.convert_into(
            &[
                100.0, 1_000.0, // frame 0
                200.0, 2_000.0, // frame 1
                300.0, 3_000.0, // frame 2
                400.0, 4_000.0, // frame 3
            ],
            &mut producer,
            &mut scratch,
        );
        let output: Vec<f32> = consumer.pop_iter().collect();

        assert_eq!(output.len(), 6);
        for frame in output.as_chunks::<2>().0 {
            assert!(frame[0] < 500.0);
            assert!(frame[1] > 500.0);
        }
        assert!((output[0] - 100.0).abs() < f32::EPSILON);
        assert!((output[1] - 1_000.0).abs() < f32::EPSILON);
        assert!(output[2] > output[0]);
        assert!(output[3] > output[1]);
    }

    #[test]
    fn converter_keeps_fractional_phase_across_callbacks() {
        let mut converter = PassthroughConverter::new(48_000, 1, 44_100, 1);
        let (mut producer, mut consumer) = HeapRb::<f32>::new(64).split();
        let mut scratch = [0.0; IO_CHUNK_SIZE];
        converter.convert_into(&[0.0, 1.0], &mut producer, &mut scratch);
        converter.convert_into(&[2.0, 3.0, 4.0], &mut producer, &mut scratch);
        let output: Vec<f32> = consumer.pop_iter().collect();
        assert!(!output.is_empty());
        assert!(output.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(output.last().copied().unwrap_or_default() < 4.0);
    }
}

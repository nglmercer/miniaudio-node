//! Real-time Audio Passthrough Module
//! Provides low-latency audio loopback from input device to output device

use crate::conversions::convert_channels_f32;
use crate::input::AudioLevels;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{Error, Result, Status};
use napi_derive::napi;
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Callback type for audio level updates
type OnLevelsCallback = Box<dyn Fn(AudioLevels) + Send + Sync>;

const DEFAULT_SAMPLE_RATE: u32 = 44100;
const DEFAULT_CHANNELS: u16 = 1;
const DEFAULT_LATENCY_MS: u32 = 20;
const DEVICE_ID_SEPARATOR: char = ':';
const I16_MAX_F32: f32 = 32768.0;
const I8_MAX_F32: f32 = 128.0;

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
            source_buffer: Vec::new(),
            next_source_frame: 0.0,
        }
    }

    fn convert(&mut self, samples: &[f32]) -> Vec<f32> {
        let source_channels = self.source_channels as usize;
        if self.source_rate == 0
            || self.target_rate == 0
            || source_channels == 0
            || self.target_channels == 0
        {
            return Vec::new();
        }

        let complete_samples = samples.len() / source_channels * source_channels;
        self.source_buffer
            .extend_from_slice(&samples[..complete_samples]);

        let source_frames = self.source_buffer.len() / source_channels;
        let source_step = self.source_rate as f64 / self.target_rate as f64;
        let mut output = Vec::new();

        // Keep one look-ahead frame for interpolation. It is retained across
        // callbacks so both upsampling and downsampling keep their phase.
        while self.next_source_frame + 1.0 < source_frames as f64 {
            let source_index = self.next_source_frame.floor() as usize;
            let fraction = (self.next_source_frame - source_index as f64) as f32;
            let next_index = (source_index + 1).min(source_frames - 1);
            let mut frame = Vec::with_capacity(source_channels);
            for channel in 0..source_channels {
                let first = self.source_buffer[source_index * source_channels + channel];
                let second = self.source_buffer[next_index * source_channels + channel];
                frame.push(first + (second - first) * fraction);
            }

            output.extend(convert_channels_f32(
                &frame,
                self.source_channels,
                self.target_channels,
            ));
            self.next_source_frame += source_step;
        }

        // Discard source frames that can no longer be used as interpolation
        // neighbours and keep the fractional position relative to the buffer.
        let discard_frames = (self.next_source_frame.floor() as usize).saturating_sub(1);
        if discard_frames > 0 {
            let discard_samples = discard_frames * source_channels;
            self.source_buffer.drain(..discard_samples);
            self.next_source_frame -= discard_frames as f64;
        }

        output
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

    // Ring buffer for passing audio from input to output
    ring_buffer: Arc<Mutex<Option<HeapRb<f32>>>>,

    // State
    is_running: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,

    // Audio levels
    last_peak: Arc<Mutex<f64>>,
    last_rms: Arc<Mutex<f64>>,

    // Callbacks
    on_levels_callback: Arc<Mutex<Option<OnLevelsCallback>>>,
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
            ring_buffer: Arc::new(Mutex::new(None)),
            is_running: Arc::new(AtomicBool::new(false)),
            sample_rate: DEFAULT_SAMPLE_RATE,
            channels: DEFAULT_CHANNELS,
            last_peak: Arc::new(Mutex::new(0.0)),
            last_rms: Arc::new(Mutex::new(0.0)),
            on_levels_callback: Arc::new(Mutex::new(None)),
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

        *self
            .on_levels_callback
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(cb);
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

        self.sample_rate = input_config.sample_rate();
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
        let samples_per_buffer = (output_config.sample_rate() as u64
            * output_config.channels() as u64
            * target_latency as u64
            / 1000) as usize;
        let buffer_size = samples_per_buffer.max(1) * 4;
        let ring = HeapRb::<f32>::new(buffer_size as usize);

        {
            let mut rb_guard = self.ring_buffer.lock().unwrap_or_else(|e| e.into_inner());
            *rb_guard = Some(ring);
        }

        // Clone shared data
        let ring_buffer = self.ring_buffer.clone();
        let is_running = self.is_running.clone();
        let last_peak = self.last_peak.clone();
        let last_rms = self.last_rms.clone();
        let on_levels = self.on_levels_callback.clone();
        let converter = Arc::new(Mutex::new(PassthroughConverter::new(
            input_config.sample_rate(),
            input_config.channels(),
            output_config.sample_rate(),
            output_config.channels(),
        )));

        // Build input stream
        let input_stream_config: cpal::StreamConfig = input_config.clone().into();

        let err_fn = |err| {
            eprintln!("Input stream error: {}", err);
        };

        // Create input stream
        let input_stream = match input_config.sample_format() {
            cpal::SampleFormat::F32 => input_device.build_input_stream(
                &input_stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if is_running.load(Ordering::SeqCst) {
                        process_input_data(
                            data,
                            &ring_buffer,
                            &last_peak,
                            &last_rms,
                            &on_levels,
                            &converter,
                        );
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => input_device.build_input_stream(
                &input_stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if is_running.load(Ordering::SeqCst) {
                        let f32_data: Vec<f32> =
                            data.iter().map(|&s| s as f32 / I16_MAX_F32).collect();
                        process_input_data(
                            &f32_data,
                            &ring_buffer,
                            &last_peak,
                            &last_rms,
                            &on_levels,
                            &converter,
                        );
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::I8 => input_device.build_input_stream(
                &input_stream_config,
                move |data: &[i8], _: &cpal::InputCallbackInfo| {
                    if is_running.load(Ordering::SeqCst) {
                        let f32_data: Vec<f32> =
                            data.iter().map(|&s| (s as f32) / I8_MAX_F32).collect();
                        process_input_data(
                            &f32_data,
                            &ring_buffer,
                            &last_peak,
                            &last_rms,
                            &on_levels,
                            &converter,
                        );
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::U16 => input_device.build_input_stream(
                &input_stream_config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    if is_running.load(Ordering::SeqCst) {
                        let f32_data: Vec<f32> = data
                            .iter()
                            .map(|&s| ((s as i32 - I16_MAX_F32 as i32) as f32) / I16_MAX_F32)
                            .collect();
                        process_input_data(
                            &f32_data,
                            &ring_buffer,
                            &last_peak,
                            &last_rms,
                            &on_levels,
                            &converter,
                        );
                    }
                },
                err_fn,
                None,
            ),
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
        let ring_buffer_out = self.ring_buffer.clone();
        let is_running_out = self.is_running.clone();
        let output_stream = match output_config.sample_format() {
            cpal::SampleFormat::F32 => output_device.build_output_stream(
                &output_stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    fill_output_f32(data, &ring_buffer_out, &is_running_out);
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            ),
            cpal::SampleFormat::I16 => output_device.build_output_stream(
                &output_stream_config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    fill_output_i16(data, &ring_buffer_out, &is_running_out);
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            ),
            cpal::SampleFormat::I8 => output_device.build_output_stream(
                &output_stream_config,
                move |data: &mut [i8], _: &cpal::OutputCallbackInfo| {
                    fill_output_i8(data, &ring_buffer_out, &is_running_out);
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            ),
            cpal::SampleFormat::U8 => output_device.build_output_stream(
                &output_stream_config,
                move |data: &mut [u8], _: &cpal::OutputCallbackInfo| {
                    fill_output_u8(data, &ring_buffer_out, &is_running_out);
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            ),
            cpal::SampleFormat::U16 => output_device.build_output_stream(
                &output_stream_config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    fill_output_u16(data, &ring_buffer_out, &is_running_out);
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            ),
            cpal::SampleFormat::I32 => output_device.build_output_stream(
                &output_stream_config,
                move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                    fill_output_i32(data, &ring_buffer_out, &is_running_out);
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            ),
            cpal::SampleFormat::U32 => output_device.build_output_stream(
                &output_stream_config,
                move |data: &mut [u32], _: &cpal::OutputCallbackInfo| {
                    fill_output_u32(data, &ring_buffer_out, &is_running_out);
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            ),
            cpal::SampleFormat::F64 => output_device.build_output_stream(
                &output_stream_config,
                move |data: &mut [f64], _: &cpal::OutputCallbackInfo| {
                    fill_output_f64(data, &ring_buffer_out, &is_running_out);
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            ),
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

        // Clear ring buffer
        {
            let mut rb_guard = self.ring_buffer.lock().unwrap_or_else(|e| e.into_inner());
            *rb_guard = None;
        }

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
            peak: *self.last_peak.lock().unwrap_or_else(|e| e.into_inner()),
            rms: *self.last_rms.lock().unwrap_or_else(|e| e.into_inner()),
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
        let host = cpal::default_host();

        // Get the default output device for comparison
        let default_device = host.default_output_device();

        if let Ok(devices) = host.output_devices() {
            for (i, device) in devices.enumerate() {
                if let Ok(desc) = device.description() {
                    let name = desc.name();
                    // Skip null/discard devices
                    let name_lower = name.to_lowercase();
                    if name_lower.contains("null") || name_lower.contains("discard") {
                        continue;
                    }

                    // Check if this is the default device
                    let is_default = default_device.as_ref().is_some_and(|d| {
                        d.description().map(|dd| dd.name() == name).unwrap_or(false)
                    });

                    result.push(crate::types::AudioDeviceInfo {
                        id: format!("{}:{}", host.id(), i),
                        name: name.to_string(),
                        host: format!("{:?}", host.id()),
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
                if id.contains(DEVICE_ID_SEPARATOR) {
                    let parts: Vec<&str> = id.split(DEVICE_ID_SEPARATOR).collect();
                    let host_name = parts[0];
                    let device_idx = parts[1].parse::<usize>().map_err(|_| {
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
                    // Try to parse as index
                    let idx = id.parse::<usize>().unwrap_or(0);
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
                if id.contains(DEVICE_ID_SEPARATOR) {
                    let parts: Vec<&str> = id.split(DEVICE_ID_SEPARATOR).collect();
                    let host_name = parts[0];
                    let device_idx = parts[1].parse::<usize>().map_err(|_| {
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
                    let idx = id.parse::<usize>().unwrap_or(0);
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
    ring_buffer: &Arc<Mutex<Option<HeapRb<f32>>>>,
    last_peak: &Arc<Mutex<f64>>,
    last_rms: &Arc<Mutex<f64>>,
    on_levels: &Arc<Mutex<Option<OnLevelsCallback>>>,
    converter: &Arc<Mutex<PassthroughConverter>>,
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
    {
        let mut peak_guard = last_peak.lock().unwrap_or_else(|e| e.into_inner());
        *peak_guard = peak as f64;
    }
    {
        let mut rms_guard = last_rms.lock().unwrap_or_else(|e| e.into_inner());
        *rms_guard = rms as f64;
    }

    // Emit callback
    {
        let callback_guard = on_levels.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cb) = callback_guard.as_ref() {
            cb(AudioLevels {
                peak: peak as f64,
                rms: rms as f64,
            });
        }
    }

    // Push to ring buffer
    {
        let converted = converter
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .convert(data);
        let mut rb_guard = ring_buffer.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(rb) = rb_guard.as_mut() {
            use ringbuf::traits::RingBuffer;
            // The passthrough is a live stream: newest samples should replace
            // the oldest samples if output temporarily falls behind.
            rb.push_slice_overwrite(&converted);
        }
    }
}

fn next_output_sample(
    ring_buffer: &Arc<Mutex<Option<HeapRb<f32>>>>,
    is_running: &Arc<AtomicBool>,
) -> f32 {
    if !is_running.load(Ordering::SeqCst) {
        return 0.0;
    }

    let mut rb_guard = ring_buffer.lock().unwrap_or_else(|e| e.into_inner());
    rb_guard
        .as_mut()
        .and_then(|rb| {
            use ringbuf::traits::Consumer;
            rb.try_pop()
        })
        .unwrap_or(0.0)
}

fn fill_output_f32(
    data: &mut [f32],
    ring_buffer: &Arc<Mutex<Option<HeapRb<f32>>>>,
    is_running: &Arc<AtomicBool>,
) {
    for sample in data {
        *sample = next_output_sample(ring_buffer, is_running);
    }
}

fn fill_output_i16(
    data: &mut [i16],
    ring_buffer: &Arc<Mutex<Option<HeapRb<f32>>>>,
    is_running: &Arc<AtomicBool>,
) {
    for sample in data {
        *sample =
            (next_output_sample(ring_buffer, is_running).clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
    }
}

fn fill_output_i8(
    data: &mut [i8],
    ring_buffer: &Arc<Mutex<Option<HeapRb<f32>>>>,
    is_running: &Arc<AtomicBool>,
) {
    for sample in data {
        *sample =
            (next_output_sample(ring_buffer, is_running).clamp(-1.0, 1.0) * i8::MAX as f32) as i8;
    }
}

fn fill_output_u8(
    data: &mut [u8],
    ring_buffer: &Arc<Mutex<Option<HeapRb<f32>>>>,
    is_running: &Arc<AtomicBool>,
) {
    for sample in data {
        let normalized = next_output_sample(ring_buffer, is_running).clamp(-1.0, 1.0);
        *sample = ((normalized + 1.0) * 127.5) as u8;
    }
}

fn fill_output_u16(
    data: &mut [u16],
    ring_buffer: &Arc<Mutex<Option<HeapRb<f32>>>>,
    is_running: &Arc<AtomicBool>,
) {
    for sample in data {
        let normalized = next_output_sample(ring_buffer, is_running).clamp(-1.0, 1.0);
        *sample = ((normalized + 1.0) * 32767.5) as u16;
    }
}

fn fill_output_i32(
    data: &mut [i32],
    ring_buffer: &Arc<Mutex<Option<HeapRb<f32>>>>,
    is_running: &Arc<AtomicBool>,
) {
    for sample in data {
        *sample =
            (next_output_sample(ring_buffer, is_running).clamp(-1.0, 1.0) * i32::MAX as f32) as i32;
    }
}

fn fill_output_u32(
    data: &mut [u32],
    ring_buffer: &Arc<Mutex<Option<HeapRb<f32>>>>,
    is_running: &Arc<AtomicBool>,
) {
    for sample in data {
        let normalized = next_output_sample(ring_buffer, is_running).clamp(-1.0, 1.0);
        *sample = ((normalized + 1.0) * 2_147_483_647.5) as u32;
    }
}

fn fill_output_f64(
    data: &mut [f64],
    ring_buffer: &Arc<Mutex<Option<HeapRb<f32>>>>,
    is_running: &Arc<AtomicBool>,
) {
    for sample in data {
        *sample = next_output_sample(ring_buffer, is_running) as f64;
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
        let output = converter.convert(&[
            100.0, 1_000.0, // frame 0
            200.0, 2_000.0, // frame 1
            300.0, 3_000.0, // frame 2
            400.0, 4_000.0, // frame 3
        ]);

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
        let first = converter.convert(&[0.0, 1.0]);
        let second = converter.convert(&[2.0, 3.0, 4.0]);

        let mut output = first;
        output.extend(second);
        assert!(!output.is_empty());
        assert!(output.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(output.last().copied().unwrap_or_default() < 4.0);
    }
}

//! Real-time Audio Passthrough Module
//! Provides low-latency audio loopback from input device to output device

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
            sample_rate: 44100,
            channels: 1,
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

        *self.on_levels_callback.lock().unwrap() = Some(cb);
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

        let target_latency = latency_ms.unwrap_or(20);

        // Get input device
        let host = cpal::default_host();

        // Try to find a real input device if no specific device requested
        let input_device = if input_device_id.as_ref().is_some_and(|s| !s.is_empty()) {
            self.get_input_device(&host, input_device_id.as_deref())?
        } else {
            // Find first real input device (not virtual)
            let found_device = None;
            if let Ok(devices) = host.input_devices() {
                for device in devices {
                    if let Ok(desc) = device.description() {
                        let _name = desc.name().to_lowercase();
                    }
                }
            }
            found_device
                .ok_or_else(|| Error::new(Status::GenericFailure, "No input device found"))?
        };

        // Get output device
        let output_device = self.get_output_device(&host, output_device_id.as_deref())?;

        // Get input config
        let input_config = input_device.default_input_config().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to get input config: {}", e),
            )
        })?;

        self.sample_rate = input_config.sample_rate();
        self.channels = input_config.channels();

        // Get output config - try to match input config
        let _output_config = output_device.default_output_config().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to get output config: {}", e),
            )
        })?;

        // Create ring buffer - size based on latency
        // At 44100 Hz with 2 channels, we need ~1764 samples per 20ms
        let samples_per_buffer = (self.sample_rate * self.channels as u32 * target_latency) / 1000;
        let buffer_size = samples_per_buffer * 4; // 4x for safety margin
        let ring = HeapRb::<f32>::new(buffer_size as usize);

        {
            let mut rb_guard = self.ring_buffer.lock().unwrap();
            *rb_guard = Some(ring);
        }

        // Clone shared data
        let ring_buffer = self.ring_buffer.clone();
        let is_running = self.is_running.clone();
        let last_peak = self.last_peak.clone();
        let last_rms = self.last_rms.clone();
        let on_levels = self.on_levels_callback.clone();

        // Build input stream
        let stream_config: cpal::StreamConfig = input_config.clone().into();

        let err_fn = |err| {
            eprintln!("Input stream error: {}", err);
        };

        // Create input stream
        let input_stream = match input_config.sample_format() {
            cpal::SampleFormat::F32 => input_device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if is_running.load(Ordering::SeqCst) {
                        process_input_data(data, &ring_buffer, &last_peak, &last_rms, &on_levels);
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => input_device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if is_running.load(Ordering::SeqCst) {
                        let f32_data: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                        process_input_data(
                            &f32_data,
                            &ring_buffer,
                            &last_peak,
                            &last_rms,
                            &on_levels,
                        );
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::I8 => input_device.build_input_stream(
                &stream_config,
                move |data: &[i8], _: &cpal::InputCallbackInfo| {
                    if is_running.load(Ordering::SeqCst) {
                        let f32_data: Vec<f32> = data.iter().map(|&s| (s as f32) / 128.0).collect();
                        process_input_data(
                            &f32_data,
                            &ring_buffer,
                            &last_peak,
                            &last_rms,
                            &on_levels,
                        );
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::U16 => input_device.build_input_stream(
                &stream_config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    if is_running.load(Ordering::SeqCst) {
                        let f32_data: Vec<f32> = data
                            .iter()
                            .map(|&s| ((s as i32 - 32768) as f32) / 32768.0)
                            .collect();
                        process_input_data(
                            &f32_data,
                            &ring_buffer,
                            &last_peak,
                            &last_rms,
                            &on_levels,
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

        // Clone for output stream
        let ring_buffer_out = self.ring_buffer.clone();
        let is_running_out = self.is_running.clone();

        // Build output stream with matching config
        let output_stream = output_device
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if is_running_out.load(Ordering::SeqCst) {
                        let mut rb_guard = ring_buffer_out.lock().unwrap();
                        if let Some(rb) = rb_guard.as_mut() {
                            // Use pop_iter to get samples - it handles available samples internally
                            use ringbuf::traits::Consumer;
                            let mut pop_iter = rb.pop_iter();
                            // Try to fill the output buffer
                            for sample in data.iter_mut() {
                                *sample = pop_iter.next().unwrap_or(0.0);
                            }
                        } else {
                            // No ring buffer, output silence
                            for sample in data.iter_mut() {
                                *sample = 0.0;
                            }
                        }
                    } else {
                        // Not running, output silence
                        for sample in data.iter_mut() {
                            *sample = 0.0;
                        }
                    }
                },
                |err| {
                    eprintln!("Output stream error: {}", err);
                },
                None,
            )
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
            let mut rb_guard = self.ring_buffer.lock().unwrap();
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
            peak: *self.last_peak.lock().unwrap(),
            rms: *self.last_rms.lock().unwrap(),
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
                if id.contains(':') {
                    let parts: Vec<&str> = id.split(':').collect();
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
                if id.contains(':') {
                    let parts: Vec<&str> = id.split(':').collect();
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
        let mut peak_guard = last_peak.lock().unwrap();
        *peak_guard = peak as f64;
    }
    {
        let mut rms_guard = last_rms.lock().unwrap();
        *rms_guard = rms as f64;
    }

    // Emit callback
    {
        let callback_guard = on_levels.lock().unwrap();
        if let Some(cb) = callback_guard.as_ref() {
            cb(AudioLevels {
                peak: peak as f64,
                rms: rms as f64,
            });
        }
    }

    // Push to ring buffer
    {
        let mut rb_guard = ring_buffer.lock().unwrap();
        if let Some(rb) = rb_guard.as_mut() {
            use ringbuf::traits::Producer;
            // Use push_slice for more efficient bulk insert
            let _ = rb.push_slice(data);
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

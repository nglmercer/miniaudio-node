use crate::buffer::SamplesBuffer;
use crate::types::AudioDeviceInfo;
use arc_swap::ArcSwapOption;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::cpal;
use rodio::cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rodio::cpal::Sample;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
type OnDataCallback = Box<dyn Fn(Vec<i16>) + Send + Sync>;

const DEFAULT_SAMPLE_RATE: u32 = 44100;
const DEFAULT_CHANNELS: u16 = 1;
const DEFAULT_RESERVE_SECONDS: u32 = 10;
const DEVICE_ID_SEPARATOR: char = ':';
const I16_MAX_F32: f32 = 32768.0;
const INPUT_CHUNK_SIZE: usize = 4096;
#[cfg(target_os = "linux")]
const PREFERRED_LINUX_BUFFER_SIZE: u32 = 1024;

/// A fixed-capacity chronological history for samples arriving from the audio
/// callback. Unlike an ordinary SPSC queue, writes overwrite the oldest
/// samples once the history is full.
struct RollingSamples {
    buffer: Vec<i16>,
    write_index: usize,
    len: usize,
}

impl RollingSamples {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0; capacity],
            write_index: 0,
            len: 0,
        }
    }

    /// Append samples without allocating. If the input is larger than the
    /// history, only its newest capacity-sized suffix is written.
    fn push_slice(&mut self, data: &[i16]) {
        let capacity = self.buffer.len();
        if capacity == 0 {
            return;
        }

        let data = if data.len() > capacity {
            &data[data.len() - capacity..]
        } else {
            data
        };

        for &sample in data {
            self.buffer[self.write_index] = sample;
            self.write_index += 1;
            if self.write_index == capacity {
                self.write_index = 0;
            }
            self.len = (self.len + 1).min(capacity);
        }
    }

    /// Reconstruct the retained samples from oldest to newest. This allocates
    /// only for the caller’s snapshot, never for audio callback writes.
    fn snapshot(&self) -> Vec<i16> {
        if self.len == 0 {
            return Vec::new();
        }

        let capacity = self.buffer.len();
        let oldest = (self.write_index + capacity - self.len) % capacity;
        let first_len = self.len.min(capacity - oldest);
        let mut snapshot = Vec::with_capacity(self.len);
        snapshot.extend_from_slice(&self.buffer[oldest..oldest + first_len]);
        if first_len < self.len {
            snapshot.extend_from_slice(&self.buffer[..self.len - first_len]);
        }
        snapshot
    }

    fn clear(&mut self) {
        self.write_index = 0;
        self.len = 0;
    }
}

#[derive(Default)]
struct RecorderHistory {
    samples: Vec<i16>,
    rolling: Option<RollingSamples>,
}

impl RecorderHistory {
    fn push_slice(&mut self, data: &[i16]) {
        if let Some(rolling) = self.rolling.as_mut() {
            rolling.push_slice(data);
        } else {
            self.samples.extend_from_slice(data);
        }
    }

    fn snapshot(&self) -> Vec<i16> {
        self.rolling
            .as_ref()
            .map_or_else(|| self.samples.clone(), RollingSamples::snapshot)
    }
}

fn process_typed_recording<T: Copy, F: FnMut(&[i16])>(
    data: &[T],
    scratch: &mut [i16; INPUT_CHUNK_SIZE],
    convert: fn(T) -> i16,
    process: &mut F,
) {
    for chunk in data.chunks(INPUT_CHUNK_SIZE) {
        for (index, sample) in chunk.iter().copied().enumerate() {
            scratch[index] = convert(sample);
        }
        process(&scratch[..chunk.len()]);
    }
}

fn f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * I16_MAX_F32) as i16
}

fn f64_to_i16(sample: f64) -> i16 {
    (sample.clamp(-1.0, 1.0) * I16_MAX_F32 as f64) as i16
}

fn i8_to_i16(sample: i8) -> i16 {
    (sample as i16) << 8
}

fn i24_to_i16(sample: cpal::I24) -> i16 {
    sample.to_sample()
}

fn i32_to_i16(sample: i32) -> i16 {
    (sample >> 16) as i16
}

fn i64_to_i16(sample: i64) -> i16 {
    (sample >> 48) as i16
}

fn u8_to_i16(sample: u8) -> i16 {
    ((sample as i16) - 128) << 8
}

fn u16_to_i16(sample: u16) -> i16 {
    (sample as i32 - 32_768) as i16
}

fn u32_to_i16(sample: u32) -> i16 {
    ((sample as i64 - 2_147_483_648) >> 16) as i16
}

fn u64_to_i16(sample: u64) -> i16 {
    ((sample as i128 - 9_223_372_036_854_775_808i128) >> 48) as i16
}

#[napi(object)]
pub struct AudioHostInfo {
    pub id: String,
    pub name: String,
}

#[napi(object)]
pub struct RecorderConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
}

#[napi(object)]
pub struct AudioLevels {
    pub peak: f64,
    pub rms: f64,
}

#[napi]
pub fn get_available_hosts() -> Vec<AudioHostInfo> {
    cpal::available_hosts()
        .iter()
        .map(|h| {
            let id = format!("{:?}", h);
            let name = match id.to_lowercase().as_str() {
                "alsa" => "ALSA (Linux Standard)".to_string(),
                "jack" => "JACK (Professional Audio)".to_string(),
                "asio" => "ASIO (Windows Pro Audio)".to_string(),
                "wasapi" => "WASAPI (Windows Standard)".to_string(),
                "coreaudio" => "CoreAudio (macOS Standard)".to_string(),
                _ => id.clone(),
            };
            AudioHostInfo { id, name }
        })
        .collect()
}

fn create_device_info(
    host_name: &str,
    index: usize,
    device: &cpal::Device,
    default_name: &Option<String>,
) -> Option<AudioDeviceInfo> {
    let name = device.name().ok()?;
    // Only include devices that actually have a default input config
    if device.default_input_config().is_err() {
        return None;
    }

    Some(AudioDeviceInfo {
        id: format!("{}{}{}", host_name, DEVICE_ID_SEPARATOR, index),
        name: name.clone(),
        host: host_name.to_string(),
        is_default: Some(name) == *default_name,
    })
}

#[napi]
pub fn get_input_devices_by_host(host_name: String) -> Result<Vec<AudioDeviceInfo>> {
    let host_id = cpal::available_hosts()
        .into_iter()
        .find(|h| format!("{:?}", h) == host_name)
        .ok_or_else(|| {
            Error::new(
                Status::InvalidArg,
                format!("Host '{}' not found", host_name),
            )
        })?;

    let host = cpal::host_from_id(host_id)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    let devices = host
        .input_devices()
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    let default_name = host.default_input_device().and_then(|d| d.name().ok());

    let result = devices
        .enumerate()
        .filter_map(|(i, device)| create_device_info(&host_name, i, &device, &default_name))
        .collect();

    Ok(result)
}

#[napi]
pub fn get_input_devices() -> Result<Vec<AudioDeviceInfo>> {
    let mut result = Vec::new();
    let available_hosts = cpal::available_hosts();

    for host_id in available_hosts {
        let host = match cpal::host_from_id(host_id) {
            Ok(h) => h,
            Err(_) => continue,
        };

        let host_name = format!("{:?}", host_id);
        let devices = match host.input_devices() {
            Ok(d) => d,
            Err(_) => continue,
        };

        let default_name = host.default_input_device().and_then(|d| d.name().ok());

        for (i, device) in devices.enumerate() {
            if let Some(info) = create_device_info(&host_name, i, &device, &default_name) {
                result.push(info);
            }
        }
    }

    if result.is_empty() {
        // Fallback to default host if everything else failed
        let host = cpal::default_host();
        let host_name = format!("{:?}", host.id());
        if let Ok(devices) = host.input_devices() {
            let default_name = host.default_input_device().and_then(|d| d.name().ok());
            for (i, device) in devices.enumerate() {
                if let Some(info) = create_device_info(&host_name, i, &device, &default_name) {
                    result.push(info);
                }
            }
        }
    }

    Ok(result)
}

#[napi]
pub struct AudioRecorder {
    stream: Option<cpal::Stream>,
    // The Vec backs unbounded getBuffer() history; rolling is the fixed-
    // capacity, overwrite-on-overflow history used by both public snapshots.
    // The active store is updated while holding this one existing history lock.
    recorded_history: Arc<Mutex<RecorderHistory>>,
    recorded_capacity: Arc<Mutex<Option<usize>>>,
    on_data_callback: Arc<ArcSwapOption<OnDataCallback>>,
    is_recording: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
    last_peak: Arc<AtomicU64>,
    last_rms: Arc<AtomicU64>,
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[napi]
impl AudioRecorder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            stream: None,
            recorded_history: Arc::new(Mutex::new(RecorderHistory::default())),
            recorded_capacity: Arc::new(Mutex::new(None)),
            on_data_callback: Arc::new(ArcSwapOption::empty()),
            is_recording: Arc::new(AtomicBool::new(false)),
            sample_rate: DEFAULT_SAMPLE_RATE,
            channels: DEFAULT_CHANNELS,
            last_peak: Arc::new(AtomicU64::new(0.0f64.to_bits())),
            last_rms: Arc::new(AtomicU64::new(0.0f64.to_bits())),
        }
    }

    #[napi]
    pub fn set_on_data(&self, callback: ThreadsafeFunction<Vec<i16>>) -> Result<()> {
        let cb = Box::new(move |data: Vec<i16>| {
            callback.call(
                Ok::<_, Error>(data),
                ThreadsafeFunctionCallMode::NonBlocking,
            );
        });

        self.on_data_callback.store(Some(Arc::new(cb)));
        Ok(())
    }

    #[napi]
    pub fn set_ring_buffer_size(&self, size_samples: u32) -> Result<()> {
        if self.is_recording.load(Ordering::Acquire) {
            return Err(Error::new(
                Status::GenericFailure,
                "Ring buffer size cannot change while recording is active",
            ));
        }
        if size_samples == 0 {
            return Err(Error::new(
                Status::InvalidArg,
                "Ring buffer size must be greater than zero",
            ));
        }
        let capacity = size_samples as usize;
        *self
            .recorded_capacity
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(capacity);
        let mut history = self
            .recorded_history
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let existing = history.snapshot();
        let mut rolling = RollingSamples::new(capacity);
        rolling.push_slice(&existing);
        history.samples.clear();
        history.rolling = Some(rolling);
        Ok(())
    }

    #[napi]
    pub fn start(&mut self, device_id: Option<String>) -> Result<()> {
        if self.is_recording.load(Ordering::SeqCst) {
            return Err(Error::new(
                Status::GenericFailure,
                "Recording is already in progress",
            ));
        }

        let host = cpal::default_host();
        let device = if let Some(id) = device_id {
            if let Some((host_name, index)) = id.split_once(DEVICE_ID_SEPARATOR) {
                let device_idx = index.parse::<usize>().map_err(|_| {
                    Error::new(
                        Status::InvalidArg,
                        format!("Invalid device index in ID: {}", id),
                    )
                })?;

                let host_id = cpal::available_hosts()
                    .into_iter()
                    .find(|h| format!("{:?}", h) == host_name)
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
                            format!(
                                "Device at index {} not found on host {}",
                                device_idx, host_name
                            ),
                        )
                    })?
            } else {
                // Keep supporting legacy numeric IDs, but reject malformed
                // values instead of silently selecting device 0.
                let device_idx = id.parse::<usize>().map_err(|_| {
                    Error::new(Status::InvalidArg, format!("Invalid device ID: {}", id))
                })?;
                host.input_devices()
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                    .nth(device_idx)
                    .ok_or_else(|| {
                        Error::new(Status::InvalidArg, format!("Device ID {} not found", id))
                    })?
            }
        } else {
            host.default_input_device().ok_or_else(|| {
                Error::new(Status::GenericFailure, "No default input device available")
            })?
        };

        let config = device.default_input_config().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to get default input config: {}", e),
            )
        })?;

        self.sample_rate = config.sample_rate().0;
        self.channels = config.channels();

        let recorded_history = self.recorded_history.clone();
        let recorded_capacity = self.recorded_capacity.clone();
        let on_data = self.on_data_callback.clone();
        let is_recording = self.is_recording.clone();
        let last_peak = self.last_peak.clone();
        let last_rms = self.last_rms.clone();

        let retention = *recorded_capacity.lock().unwrap_or_else(|e| e.into_inner());

        // Reserve for 10 seconds of audio by default.
        {
            let mut history = recorded_history.lock().unwrap_or_else(|e| e.into_inner());
            history.samples.clear();
            history.rolling = retention.map(RollingSamples::new);
            let reserve_size = retention.unwrap_or(
                (self.sample_rate * self.channels as u32 * DEFAULT_RESERVE_SECONDS) as usize,
            );
            history.samples.reserve(reserve_size);
        }

        let err_fn = move |err| {
            eprintln!("Audio stream error: {}", err);
        };

        let mut stream_config: cpal::StreamConfig = config.clone().into();
        stream_config.buffer_size = cpal::BufferSize::Default;

        #[cfg(target_os = "linux")]
        {
            if let cpal::SupportedBufferSize::Range { min, max } = config.buffer_size() {
                let preferred = PREFERRED_LINUX_BUFFER_SIZE;
                if preferred >= *min && preferred <= *max {
                    stream_config.buffer_size = cpal::BufferSize::Fixed(preferred);
                }
            }
        }

        let mut process_samples = move |data: &[i16]| {
            if is_recording.load(Ordering::SeqCst) {
                // Calculate stats
                let mut peak: f32 = 0.0;
                let mut sum_sq: f64 = 0.0;
                for &s in data {
                    let val = (s as f32 / I16_MAX_F32).abs();
                    if val > peak {
                        peak = val;
                    }
                    sum_sq += (val * val) as f64;
                }

                last_peak.store((peak as f64).to_bits(), Ordering::Relaxed);
                let rms = if data.is_empty() {
                    0.0
                } else {
                    (sum_sq / data.len() as f64).sqrt()
                };
                last_rms.store(rms.to_bits(), Ordering::Relaxed);

                // Fill full history
                {
                    let mut history = recorded_history.lock().unwrap_or_else(|e| e.into_inner());
                    history.push_slice(data);
                }

                // Emit callback
                if let Some(cb) = on_data.load().as_ref() {
                    cb(data.to_vec());
                }
            }
        };

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let mut converted = [0i16; INPUT_CHUNK_SIZE];
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        process_typed_recording(
                            data,
                            &mut converted,
                            f32_to_i16,
                            &mut process_samples,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I16 => device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    process_samples(data);
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::I8 => {
                let mut converted = [0i16; INPUT_CHUNK_SIZE];
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i8], _: &cpal::InputCallbackInfo| {
                        process_typed_recording(
                            data,
                            &mut converted,
                            i8_to_i16,
                            &mut process_samples,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I24 => {
                let mut converted = [0i16; INPUT_CHUNK_SIZE];
                device.build_input_stream(
                    &stream_config,
                    move |data: &[cpal::I24], _: &cpal::InputCallbackInfo| {
                        process_typed_recording(
                            data,
                            &mut converted,
                            i24_to_i16,
                            &mut process_samples,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I32 => {
                let mut converted = [0i16; INPUT_CHUNK_SIZE];
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i32], _: &cpal::InputCallbackInfo| {
                        process_typed_recording(
                            data,
                            &mut converted,
                            i32_to_i16,
                            &mut process_samples,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I64 => {
                let mut converted = [0i16; INPUT_CHUNK_SIZE];
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i64], _: &cpal::InputCallbackInfo| {
                        process_typed_recording(
                            data,
                            &mut converted,
                            i64_to_i16,
                            &mut process_samples,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U8 => {
                let mut converted = [0i16; INPUT_CHUNK_SIZE];
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u8], _: &cpal::InputCallbackInfo| {
                        process_typed_recording(
                            data,
                            &mut converted,
                            u8_to_i16,
                            &mut process_samples,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let mut converted = [0i16; INPUT_CHUNK_SIZE];
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        process_typed_recording(
                            data,
                            &mut converted,
                            u16_to_i16,
                            &mut process_samples,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U32 => {
                let mut converted = [0i16; INPUT_CHUNK_SIZE];
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u32], _: &cpal::InputCallbackInfo| {
                        process_typed_recording(
                            data,
                            &mut converted,
                            u32_to_i16,
                            &mut process_samples,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U64 => {
                let mut converted = [0i16; INPUT_CHUNK_SIZE];
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u64], _: &cpal::InputCallbackInfo| {
                        process_typed_recording(
                            data,
                            &mut converted,
                            u64_to_i16,
                            &mut process_samples,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::F64 => {
                let mut converted = [0i16; INPUT_CHUNK_SIZE];
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f64], _: &cpal::InputCallbackInfo| {
                        process_typed_recording(
                            data,
                            &mut converted,
                            f64_to_i16,
                            &mut process_samples,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            _ => {
                return Err(Error::new(
                    Status::GenericFailure,
                    format!("Unsupported sample format: {:?}", config.sample_format()),
                ))
            }
        }
        .map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to build input stream: {}", e),
            )
        })?;

        stream.play().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to start input stream: {}", e),
            )
        })?;

        self.stream = Some(stream);
        self.is_recording.store(true, Ordering::SeqCst);

        Ok(())
    }

    #[napi]
    pub fn stop(&mut self) -> Result<()> {
        if !self.is_recording.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.is_recording.store(false, Ordering::SeqCst);
        self.stream = None;

        Ok(())
    }

    #[napi]
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    #[napi]
    pub fn get_buffer(&self) -> Result<SamplesBuffer> {
        let samples = self
            .recorded_history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot();
        if self.channels == 0 || self.sample_rate == 0 {
            return Err(Error::new(
                Status::GenericFailure,
                "Recorder has an invalid sample format",
            ));
        }
        let channels = self.channels as usize;
        let complete_len = samples.len() - samples.len() % channels;
        let samples = samples[samples.len() - complete_len..].to_vec();
        SamplesBuffer::create(self.channels as u32, self.sample_rate, samples)
    }

    #[napi]
    pub fn get_ring_buffer_samples(&self) -> Result<Vec<i16>> {
        let history = self
            .recorded_history
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        Ok(history
            .rolling
            .as_ref()
            .map_or_else(Vec::new, RollingSamples::snapshot))
    }

    #[napi]
    pub fn clear(&mut self) {
        let mut history = self
            .recorded_history
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        history.samples.clear();
        if let Some(rolling) = history.rolling.as_mut() {
            rolling.clear();
        }
        self.last_peak.store(0.0f64.to_bits(), Ordering::Relaxed);
        self.last_rms.store(0.0f64.to_bits(), Ordering::Relaxed);
    }

    #[napi]
    pub fn get_config(&self) -> RecorderConfig {
        RecorderConfig {
            sample_rate: self.sample_rate,
            channels: self.channels,
            sample_format: "i16".to_string(), // We normalize everything to i16
        }
    }

    #[napi]
    pub fn get_levels(&self) -> AudioLevels {
        AudioLevels {
            peak: f64::from_bits(self.last_peak.load(Ordering::Relaxed)),
            rms: f64::from_bits(self.last_rms.load(Ordering::Relaxed)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_history_keeps_samples_under_capacity() {
        let mut history = RollingSamples::new(4);
        history.push_slice(&[1, 2]);
        assert_eq!(history.snapshot(), vec![1, 2]);
    }

    #[test]
    fn rolling_history_keeps_samples_at_exact_capacity() {
        let mut history = RollingSamples::new(4);
        history.push_slice(&[1, 2, 3, 4]);
        assert_eq!(history.snapshot(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn rolling_history_overflow_keeps_the_newest_samples() {
        let mut history = RollingSamples::new(4);
        history.push_slice(&[1, 2, 3, 4]);
        history.push_slice(&[5, 6]);
        assert_eq!(history.snapshot(), vec![3, 4, 5, 6]);
    }

    #[test]
    fn rolling_history_truncates_an_oversized_callback_chunk() {
        let mut history = RollingSamples::new(4);
        history.push_slice(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(history.snapshot(), vec![3, 4, 5, 6]);
    }

    #[test]
    fn rolling_history_snapshots_are_non_destructive() {
        let mut history = RollingSamples::new(4);
        history.push_slice(&[1, 2, 3, 4]);
        let first = history.snapshot();
        let second = history.snapshot();
        assert_eq!(first, vec![1, 2, 3, 4]);
        assert_eq!(second, first);
        assert_eq!(history.snapshot(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn rolling_history_clear_resets_and_accepts_new_samples() {
        let mut history = RollingSamples::new(4);
        history.push_slice(&[1, 2, 3, 4]);
        history.clear();
        assert!(history.snapshot().is_empty());
        history.push_slice(&[5, 6]);
        assert_eq!(history.snapshot(), vec![5, 6]);
    }

    #[test]
    fn recorder_ring_snapshots_and_clear_reset_all_retained_state() {
        let mut recorder = AudioRecorder::new();
        recorder.set_ring_buffer_size(4).unwrap();

        {
            let mut history = recorder
                .recorded_history
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            history.push_slice(&[1, 2, 3, 4]);
        }

        assert_eq!(
            recorder.get_ring_buffer_samples().unwrap(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(
            recorder.get_buffer().unwrap().get_samples(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(
            recorder.get_ring_buffer_samples().unwrap(),
            vec![1, 2, 3, 4]
        );

        recorder
            .recorded_history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_slice(&[5, 6]);
        assert_eq!(
            recorder.get_ring_buffer_samples().unwrap(),
            vec![3, 4, 5, 6]
        );
        assert_eq!(
            recorder.get_buffer().unwrap().get_samples(),
            vec![3, 4, 5, 6]
        );
        recorder
            .last_peak
            .store(0.75f64.to_bits(), Ordering::Relaxed);
        recorder.last_rms.store(0.5f64.to_bits(), Ordering::Relaxed);

        recorder.clear();

        assert!(recorder
            .recorded_history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .samples
            .is_empty());
        assert!(recorder
            .recorded_history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rolling
            .as_ref()
            .unwrap()
            .snapshot()
            .is_empty());
        assert!(recorder.get_ring_buffer_samples().unwrap().is_empty());
        assert_eq!(recorder.get_levels().peak, 0.0);
        assert_eq!(recorder.get_levels().rms, 0.0);
    }
}

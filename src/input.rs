use crate::buffer::SamplesBuffer;
use crate::types::AudioDeviceInfo;
use arc_swap::ArcSwapOption;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{Error, Result, Status};
use napi_derive::napi;
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapRb};
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

pub(crate) fn append_bounded(target: &mut Vec<i16>, data: &[i16], capacity: Option<usize>) {
    let Some(capacity) = capacity else {
        target.extend_from_slice(data);
        return;
    };
    if capacity == 0 {
        target.clear();
        return;
    }

    if data.len() >= capacity {
        target.clear();
        target.extend_from_slice(&data[data.len() - capacity..]);
        return;
    }

    let required_drop = target
        .len()
        .saturating_add(data.len())
        .saturating_sub(capacity);
    if required_drop > 0 {
        target.drain(..required_drop.min(target.len()));
    }
    target.extend_from_slice(data);
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
    // Full history when no retention limit is requested. Once a ring-buffer
    // size is configured this vector keeps the same bounded latest-samples
    // window instead of growing for the lifetime of the recorder.
    recorded_samples: Arc<Mutex<Vec<i16>>>,
    ring_buffer_consumer: Arc<Mutex<Option<HeapCons<i16>>>>,
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
            recorded_samples: Arc::new(Mutex::new(Vec::new())),
            ring_buffer_consumer: Arc::new(Mutex::new(None)),
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
        let (_, consumer) = HeapRb::<i16>::new(size_samples as usize).split();
        *self
            .ring_buffer_consumer
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(consumer);
        *self
            .recorded_capacity
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(size_samples as usize);
        let capacity = size_samples as usize;
        let mut recorded = self
            .recorded_samples
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if recorded.len() > capacity {
            let first = recorded.len() - capacity;
            recorded.drain(..first);
        }
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

        let recorded_samples = self.recorded_samples.clone();
        let ring_buffer_consumer = self.ring_buffer_consumer.clone();
        let recorded_capacity = self.recorded_capacity.clone();
        let on_data = self.on_data_callback.clone();
        let is_recording = self.is_recording.clone();
        let last_peak = self.last_peak.clone();
        let last_rms = self.last_rms.clone();

        let retention = *recorded_capacity.lock().unwrap_or_else(|e| e.into_inner());
        let mut ring_producer = retention.map(|capacity| {
            let (producer, consumer) = HeapRb::<i16>::new(capacity).split();
            *ring_buffer_consumer
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(consumer);
            producer
        });

        // Reserve for 10 seconds of audio by default.
        {
            let mut samples = recorded_samples.lock().unwrap_or_else(|e| e.into_inner());
            samples.clear();
            let reserve_size = retention.unwrap_or(
                (self.sample_rate * self.channels as u32 * DEFAULT_RESERVE_SECONDS) as usize,
            );
            samples.reserve(reserve_size);
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
                    let mut samples = recorded_samples.lock().unwrap_or_else(|e| e.into_inner());
                    append_bounded(&mut samples, data, retention);
                }

                // Fill ring buffer
                if let Some(producer) = ring_producer.as_mut() {
                    // A split producer performs a bounded, lock-free chunk
                    // write. If the consumer falls behind, only the newest
                    // chunk that fits is retained; the callback never blocks.
                    let _ = producer.push_slice(data);
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
            .recorded_samples
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
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
        let rb_guard = self
            .ring_buffer_consumer
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(rb) = rb_guard.as_ref() {
            // Snapshot semantics: reading recent samples does not advance the
            // consumer, so repeated reads return the same retained history.
            let samples: Vec<i16> = rb.iter().copied().collect();
            Ok(samples)
        } else {
            Ok(Vec::new())
        }
    }

    #[napi]
    pub fn clear(&mut self) {
        self.recorded_samples
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        if let Some(rb) = self
            .ring_buffer_consumer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            rb.clear();
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
    fn ring_buffer_reads_are_snapshots_and_clear_resets_all_retained_state() {
        let mut recorder = AudioRecorder::new();
        recorder.set_ring_buffer_size(4).unwrap();

        let (mut producer, consumer) = HeapRb::<i16>::new(4).split();
        *recorder
            .ring_buffer_consumer
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(consumer);
        producer.push_slice(&[1, 2, 3, 4]);

        assert_eq!(
            recorder.get_ring_buffer_samples().unwrap(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(
            recorder.get_ring_buffer_samples().unwrap(),
            vec![1, 2, 3, 4]
        );

        recorder
            .recorded_samples
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(&[5, 6]);
        recorder
            .last_peak
            .store(0.75f64.to_bits(), Ordering::Relaxed);
        recorder.last_rms.store(0.5f64.to_bits(), Ordering::Relaxed);

        recorder.clear();

        assert!(recorder
            .recorded_samples
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty());
        assert!(recorder.get_ring_buffer_samples().unwrap().is_empty());
        assert_eq!(recorder.get_levels().peak, 0.0);
        assert_eq!(recorder.get_levels().rms, 0.0);
    }
}

use crate::buffer::SamplesBuffer;
use crate::types::AudioDeviceInfo;
use arc_swap::ArcSwapOption;
use napi::bindgen_prelude::Unknown;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{Error, Result, Status};
use napi_derive::napi;
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapRb};
use rodio::cpal;
use rodio::cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rodio::cpal::Sample;
use std::sync::atomic::{AtomicBool, AtomicI16, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
type OnDataCallback = Box<dyn Fn(Vec<i16>) + Send + Sync>;

const DEFAULT_SAMPLE_RATE: u32 = 44100;
const DEFAULT_CHANNELS: u16 = 1;
const DEFAULT_RESERVE_SECONDS: u32 = 10;
const DEVICE_ID_SEPARATOR: char = ':';
const I16_MAX_F32: f32 = 32768.0;
const INPUT_CHUNK_SIZE: usize = 4096;
const ON_DATA_QUEUE_CAPACITY: usize = 8;
const ON_DATA_NAPI_QUEUE_CAPACITY: usize = 8;
#[cfg(target_os = "linux")]
const PREFERRED_LINUX_BUFFER_SIZE: u32 = 1024;

/// A fixed-capacity history whose audio writer and public snapshot readers do
/// not share a blocking mutex. Each sample is atomic so a reader can safely
/// retry a snapshot while the CPAL callback is writing newer samples.
struct AtomicRollingSamples {
    buffer: Box<[AtomicI16]>,
    write_index: AtomicUsize,
    len: AtomicUsize,
    sequence: AtomicUsize,
    clear_requested: AtomicBool,
}

impl AtomicRollingSamples {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: (0..capacity)
                .map(|_| AtomicI16::new(0))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            write_index: AtomicUsize::new(0),
            len: AtomicUsize::new(0),
            sequence: AtomicUsize::new(0),
            clear_requested: AtomicBool::new(false),
        }
    }

    /// Append samples without allocating. If the input is larger than the
    /// history, only its newest capacity-sized suffix is written.
    fn push_slice(&self, data: &[i16]) {
        let capacity = self.buffer.len();
        if capacity == 0 {
            return;
        }

        let reset = self.clear_requested.swap(false, Ordering::AcqRel);
        let data = if data.len() > capacity {
            &data[data.len() - capacity..]
        } else {
            data
        };

        // This is a single-writer sequence lock. The callback never waits for
        // readers; readers retry if a write overlaps their snapshot.
        self.sequence.fetch_add(1, Ordering::AcqRel);
        let mut write_index = if reset {
            0
        } else {
            self.write_index.load(Ordering::Relaxed)
        };
        let mut len = if reset {
            0
        } else {
            self.len.load(Ordering::Relaxed)
        };
        for &sample in data {
            self.buffer[write_index].store(sample, Ordering::Relaxed);
            write_index += 1;
            if write_index == capacity {
                write_index = 0;
            }
            len = (len + 1).min(capacity);
        }
        self.write_index.store(write_index, Ordering::Relaxed);
        self.len.store(len, Ordering::Relaxed);
        self.sequence.fetch_add(1, Ordering::Release);
    }

    /// Reconstruct the retained samples from oldest to newest. This allocates
    /// only for the caller’s snapshot, never for audio callback writes. The
    /// bounded retry loop is on the caller’s thread, never on the audio path.
    fn snapshot(&self) -> Vec<i16> {
        let capacity = self.buffer.len();
        if capacity == 0 {
            return Vec::new();
        }

        let mut retries = 0;
        loop {
            let before = self.sequence.load(Ordering::Acquire);
            if before & 1 != 0 {
                retries += 1;
                if retries % 16 == 0 {
                    thread::yield_now();
                }
                continue;
            }
            if self.clear_requested.load(Ordering::Acquire) {
                return Vec::new();
            }

            let write_index = self.write_index.load(Ordering::Relaxed);
            let len = self.len.load(Ordering::Relaxed);
            let oldest = (write_index + capacity - len) % capacity;
            let first_len = len.min(capacity - oldest);
            let mut snapshot = Vec::with_capacity(len);
            for offset in 0..first_len {
                snapshot.push(self.buffer[oldest + offset].load(Ordering::Relaxed));
            }
            for offset in first_len..len {
                snapshot.push(self.buffer[offset - first_len].load(Ordering::Relaxed));
            }

            let after = self.sequence.load(Ordering::Acquire);
            if before == after && after & 1 == 0 && !self.clear_requested.load(Ordering::Acquire) {
                return snapshot;
            }
            retries += 1;
            if retries % 16 == 0 {
                thread::yield_now();
            }
        }
    }

    fn clear(&self) {
        // The next writer folds this reset into its own sequence-locked write;
        // snapshots hide the retained values immediately.
        self.clear_requested.store(true, Ordering::Release);
    }
}

struct AudioChunk {
    len: usize,
    samples: [i16; INPUT_CHUNK_SIZE],
}

impl AudioChunk {
    fn from_slice(data: &[i16]) -> Self {
        debug_assert!(data.len() <= INPUT_CHUNK_SIZE);
        let len = data.len().min(INPUT_CHUNK_SIZE);
        let mut samples = [0; INPUT_CHUNK_SIZE];
        samples[..len].copy_from_slice(&data[..len]);
        Self { len, samples }
    }

    fn into_vec(self) -> Vec<i16> {
        self.samples[..self.len].to_vec()
    }
}

fn run_on_data_worker(
    mut consumer: HeapCons<AudioChunk>,
    on_data: Arc<ArcSwapOption<OnDataCallback>>,
    stop: Arc<AtomicBool>,
) {
    loop {
        if let Some(chunk) = consumer.try_pop() {
            if let Some(callback) = on_data.load().as_ref() {
                callback(chunk.into_vec());
            }
            continue;
        }

        if stop.load(Ordering::Acquire) {
            break;
        }
        thread::sleep(Duration::from_millis(1));
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

fn process_i16_recording<F: FnMut(&[i16])>(data: &[i16], process: &mut F) {
    for chunk in data.chunks(INPUT_CHUNK_SIZE) {
        process(chunk);
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
    // Unbounded mode retains its existing control-thread history. Bounded
    // mode uses rolling_history, which is lock-free for the audio writer and
    // snapshot readers.
    recorded_samples: Arc<Mutex<Vec<i16>>>,
    rolling_history: Arc<ArcSwapOption<AtomicRollingSamples>>,
    recorded_capacity: Arc<Mutex<Option<usize>>>,
    on_data_callback: Arc<ArcSwapOption<OnDataCallback>>,
    on_data_worker: Option<JoinHandle<()>>,
    on_data_stop: Arc<AtomicBool>,
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

impl AudioRecorder {
    fn stop_on_data_worker(&mut self) {
        self.on_data_stop.store(true, Ordering::Release);
        if let Some(worker) = self.on_data_worker.take() {
            let _ = worker.join();
        }
    }

    fn snapshot_history(&self) -> Vec<i16> {
        if let Some(rolling) = self.rolling_history.load_full() {
            rolling.snapshot()
        } else {
            self.recorded_samples
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        }
    }
}

#[napi]
impl AudioRecorder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            stream: None,
            recorded_samples: Arc::new(Mutex::new(Vec::new())),
            rolling_history: Arc::new(ArcSwapOption::empty()),
            recorded_capacity: Arc::new(Mutex::new(None)),
            on_data_callback: Arc::new(ArcSwapOption::empty()),
            on_data_worker: None,
            on_data_stop: Arc::new(AtomicBool::new(true)),
            is_recording: Arc::new(AtomicBool::new(false)),
            sample_rate: DEFAULT_SAMPLE_RATE,
            channels: DEFAULT_CHANNELS,
            last_peak: Arc::new(AtomicU64::new(0.0f64.to_bits())),
            last_rms: Arc::new(AtomicU64::new(0.0f64.to_bits())),
        }
    }

    #[napi]
    pub fn set_on_data(
        &self,
        callback: ThreadsafeFunction<
            Vec<i16>,
            Unknown<'static>,
            Vec<i16>,
            Status,
            true,
            false,
            ON_DATA_NAPI_QUEUE_CAPACITY,
        >,
    ) -> Result<()> {
        let cb = Box::new(move |data: Vec<i16>| {
            // Keep the N-API queue bounded as well as the CPAL-to-worker queue.
            // QueueFull is an intentional drop under JavaScript backpressure;
            // neither queue is allowed to make the capture path wait.
            match callback.call(
                Ok::<_, Error>(data),
                ThreadsafeFunctionCallMode::NonBlocking,
            ) {
                Status::Ok | Status::QueueFull | Status::Closing => {}
                _ => {}
            }
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
        let rolling = Arc::new(AtomicRollingSamples::new(capacity));

        if let Some(existing) = self.rolling_history.load_full() {
            // A previous bounded history is already limited to its configured
            // capacity, so this snapshot is bounded by construction.
            let existing = existing.snapshot();
            rolling.push_slice(&existing);
            *self
                .recorded_samples
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Vec::new();
        } else {
            // When switching from unbounded mode, copy only the newest tail
            // while holding the control-thread lock. Avoid cloning the whole
            // recording just to retain a smaller bounded history.
            let mut samples = self
                .recorded_samples
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let start = samples.len().saturating_sub(capacity);
            rolling.push_slice(&samples[start..]);
            *samples = Vec::new();
        }

        self.rolling_history.store(Some(rolling));
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
        let rolling_history = self.rolling_history.clone();
        let recorded_capacity = self.recorded_capacity.clone();
        let on_data = self.on_data_callback.clone();
        let is_recording = self.is_recording.clone();
        let last_peak = self.last_peak.clone();
        let last_rms = self.last_rms.clone();

        let retention = *recorded_capacity.lock().unwrap_or_else(|e| e.into_inner());

        // Allocate only the active history store. Bounded mode already owns
        // its fixed rolling storage; reserving an equally large Vec here would
        // double its memory footprint for no benefit.
        {
            let mut samples = recorded_samples.lock().unwrap_or_else(|e| e.into_inner());
            *samples = Vec::new();
            if let Some(capacity) = retention {
                rolling_history.store(Some(Arc::new(AtomicRollingSamples::new(capacity))));
            } else {
                rolling_history.store(None);
                let reserve_size =
                    (self.sample_rate * self.channels as u32 * DEFAULT_RESERVE_SECONDS) as usize;
                samples.reserve(reserve_size);
            }
        }

        let (mut on_data_producer, on_data_consumer) =
            HeapRb::<AudioChunk>::new(ON_DATA_QUEUE_CAPACITY).split();
        self.on_data_stop.store(false, Ordering::Release);
        self.on_data_worker = Some(thread::spawn({
            let on_data = on_data.clone();
            let stop = self.on_data_stop.clone();
            move || run_on_data_worker(on_data_consumer, on_data, stop)
        }));

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

                // Fill history. Bounded mode is lock-free for this callback;
                // only unbounded mode uses the control-thread Vec mutex.
                let rolling = rolling_history.load();
                if let Some(history) = rolling.as_ref() {
                    history.push_slice(data);
                } else {
                    recorded_samples
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .extend_from_slice(data);
                }

                // Queue a fixed-size, preallocated chunk. Conversion to an
                // owned Vec for N-API happens on the worker thread.
                if on_data.load().is_some() {
                    let _ = on_data_producer.try_push(AudioChunk::from_slice(data));
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
                    process_i16_recording(data, &mut process_samples);
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
                self.stop_on_data_worker();
                return Err(Error::new(
                    Status::GenericFailure,
                    format!("Unsupported sample format: {:?}", config.sample_format()),
                ));
            }
        }
        .map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to build input stream: {}", e),
            )
        });
        let stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                self.stop_on_data_worker();
                return Err(error);
            }
        };

        if let Err(error) = stream.play() {
            self.stop_on_data_worker();
            return Err(Error::new(
                Status::GenericFailure,
                format!("Failed to start input stream: {}", error),
            ));
        }

        self.stream = Some(stream);
        self.is_recording.store(true, Ordering::SeqCst);

        Ok(())
    }

    #[napi]
    pub fn stop(&mut self) -> Result<()> {
        self.is_recording.store(false, Ordering::SeqCst);
        self.stream = None;
        self.stop_on_data_worker();

        Ok(())
    }

    #[napi]
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    #[napi]
    pub fn get_buffer(&self) -> Result<SamplesBuffer> {
        let samples = self.snapshot_history();
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
        Ok(self
            .rolling_history
            .load_full()
            .map_or_else(Vec::new, |history| history.snapshot()))
    }

    #[napi]
    pub fn clear(&mut self) {
        if let Some(rolling) = self.rolling_history.load_full() {
            rolling.clear();
        } else {
            self.recorded_samples
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
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
        let history = AtomicRollingSamples::new(4);
        history.push_slice(&[1, 2]);
        assert_eq!(history.snapshot(), vec![1, 2]);
    }

    #[test]
    fn rolling_history_keeps_samples_at_exact_capacity() {
        let history = AtomicRollingSamples::new(4);
        history.push_slice(&[1, 2, 3, 4]);
        assert_eq!(history.snapshot(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn rolling_history_overflow_keeps_the_newest_samples() {
        let history = AtomicRollingSamples::new(4);
        history.push_slice(&[1, 2, 3, 4]);
        history.push_slice(&[5, 6]);
        assert_eq!(history.snapshot(), vec![3, 4, 5, 6]);
    }

    #[test]
    fn rolling_history_truncates_an_oversized_callback_chunk() {
        let history = AtomicRollingSamples::new(4);
        history.push_slice(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(history.snapshot(), vec![3, 4, 5, 6]);
    }

    #[test]
    fn rolling_history_snapshots_are_non_destructive() {
        let history = AtomicRollingSamples::new(4);
        history.push_slice(&[1, 2, 3, 4]);
        let first = history.snapshot();
        let second = history.snapshot();
        assert_eq!(first, vec![1, 2, 3, 4]);
        assert_eq!(second, first);
        assert_eq!(history.snapshot(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn rolling_history_clear_resets_and_accepts_new_samples() {
        let history = AtomicRollingSamples::new(4);
        history.push_slice(&[1, 2, 3, 4]);
        history.clear();
        assert!(history.snapshot().is_empty());
        history.push_slice(&[5, 6]);
        assert_eq!(history.snapshot(), vec![5, 6]);
    }

    #[test]
    fn rolling_history_snapshots_run_concurrently_with_writes() {
        let history = Arc::new(AtomicRollingSamples::new(1024));
        let writer_history = history.clone();
        let writer = thread::spawn(move || {
            for chunk in 0..1_000i16 {
                writer_history.push_slice(&[chunk, chunk + 1, chunk + 2, chunk + 3]);
            }
        });

        let reader_history = history.clone();
        let reader = thread::spawn(move || {
            for _ in 0..1_000 {
                assert!(reader_history.snapshot().len() <= 1024);
            }
        });

        writer.join().unwrap();
        reader.join().unwrap();
    }

    #[test]
    fn switching_from_unbounded_history_keeps_only_the_newest_tail() {
        let recorder = AudioRecorder::new();
        {
            let mut samples = recorder
                .recorded_samples
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *samples = vec![1, 2, 3, 4, 5, 6];
            assert!(samples.capacity() >= 6);
        }

        recorder.set_ring_buffer_size(4).unwrap();

        assert_eq!(
            recorder.get_ring_buffer_samples().unwrap(),
            vec![3, 4, 5, 6]
        );
        assert_eq!(
            recorder
                .recorded_samples
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .capacity(),
            0
        );
    }

    #[test]
    fn recorder_ring_snapshots_and_clear_reset_all_retained_state() {
        let mut recorder = AudioRecorder::new();
        recorder.set_ring_buffer_size(4).unwrap();

        let history = recorder.rolling_history.load_full().unwrap();
        history.push_slice(&[1, 2, 3, 4]);

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

        history.push_slice(&[5, 6]);
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
            .recorded_samples
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty());
        assert!(recorder.get_ring_buffer_samples().unwrap().is_empty());
        assert_eq!(recorder.get_levels().peak, 0.0);
        assert_eq!(recorder.get_levels().rms, 0.0);

        history.push_slice(&[7, 8]);
        assert_eq!(recorder.get_ring_buffer_samples().unwrap(), vec![7, 8]);
    }
}

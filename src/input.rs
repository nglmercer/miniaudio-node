use crate::buffer::SamplesBuffer;
use crate::types::AudioDeviceInfo;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use napi::{Error, Result, Status};
use napi_derive::napi;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[napi]
pub fn get_available_hosts() -> Vec<String> {
    cpal::available_hosts()
        .iter()
        .map(|h| format!("{:?}", h))
        .collect()
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

        let default_device = host.default_input_device();
        let default_name =
            default_device.and_then(|d| d.description().ok().map(|desc| desc.name().to_string()));

        for (i, device) in devices.enumerate() {
            if let Ok(description) = device.description() {
                // Only include devices that actually have a default input config
                if device.default_input_config().is_err() {
                    continue;
                }

                let name = description.name();
                result.push(AudioDeviceInfo {
                    // Unique ID across hosts
                    id: format!("{}:{}", host_name, i),
                    name: name.to_string(),
                    host: host_name.clone(),
                    is_default: Some(name.to_string()) == default_name,
                });
            }
        }
    }

    if result.is_empty() {
        // Fallback to default host if everything else failed
        let host = cpal::default_host();
        let host_name = format!("{:?}", host.id());
        if let Ok(devices) = host.input_devices() {
            for (i, device) in devices.enumerate() {
                if let Ok(description) = device.description() {
                    // Filter unusable devices in fallback too
                    if device.default_input_config().is_err() {
                        continue;
                    }

                    let name = description.name();
                    result.push(AudioDeviceInfo {
                        id: format!("{}:{}", host_name, i),
                        name: name.to_string(),
                        host: host_name.clone(),
                        is_default: true, // simplified
                    });
                }
            }
        }
    }

    Ok(result)
}

#[napi]
pub struct AudioRecorder {
    stream: Option<cpal::Stream>,
    recorded_samples: Arc<Mutex<Vec<i16>>>,
    is_recording: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl AudioRecorder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            stream: None,
            recorded_samples: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(AtomicBool::new(false)),
            sample_rate: 44100,
            channels: 1,
        }
    }

    #[napi]
    pub fn start(&mut self, device_id: Option<String>) -> Result<()> {
        if self.is_recording.load(Ordering::SeqCst) {
            return Err(Error::new(
                Status::GenericFailure,
                "Recording is already in progress",
            ));
        }

        // Parse host and index from ID (Format: "Host:Index")
        let (_host, device) = if let Some(id) = device_id {
            if id.contains(':') {
                let parts: Vec<&str> = id.split(':').collect();
                let host_name = parts[0];
                let device_idx = parts[1].parse::<usize>().unwrap_or(0);

                let host_id = cpal::available_hosts()
                    .into_iter()
                    .find(|h| format!("{:?}", h) == host_name)
                    .ok_or_else(|| Error::new(Status::InvalidArg, "Host not found"))?;

                let host = cpal::host_from_id(host_id)
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

                let device = host
                    .input_devices()
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                    .nth(device_idx)
                    .ok_or_else(|| Error::new(Status::InvalidArg, "Device not found"))?;

                (host, device)
            } else {
                // Fallback for old numeric IDs
                let host = cpal::default_host();
                let device = host
                    .input_devices()
                    .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
                    .nth(id.parse::<usize>().unwrap_or(0))
                    .ok_or_else(|| Error::new(Status::InvalidArg, "Device not found"))?;
                (host, device)
            }
        } else {
            let host = cpal::default_host();
            let device = host
                .default_input_device()
                .ok_or_else(|| Error::new(Status::GenericFailure, "No default input device"))?;
            (host, device)
        };

        let config = device.default_input_config().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to get default input config: {}", e),
            )
        })?;

        self.sample_rate = config.sample_rate();

        self.channels = config.channels();

        let recorded_samples = self.recorded_samples.clone();
        let is_recording = self.is_recording.clone();

        // Clear and pre-reserve memory to avoid reallocations in the audio callback.
        // We reserve for 10 seconds of audio by default.
        {
            let mut samples = recorded_samples.lock().unwrap();
            samples.clear();
            let reserve_size = (self.sample_rate * self.channels as u32 * 10) as usize;
            samples.reserve(reserve_size);
        }

        let err_fn = move |err| {
            eprintln!("an error occurred on stream: {}", err);
        };

        let mut stream_config: cpal::StreamConfig = config.clone().into();

        // Linux ALSA dynamic buffer sizes can cause "hissing" or crackling.
        // Following user advice: Try Default, but if Fixed(1024) is within supported range,
        // it's often more stable on Linux.
        stream_config.buffer_size = cpal::BufferSize::Default;

        #[cfg(target_os = "linux")]
        {
            if let cpal::SupportedBufferSize::Range { min, max } = config.buffer_size() {
                // 1024 or 512 are typically good for stability.
                let preferred = 1024;
                if preferred >= *min && preferred <= *max {
                    stream_config.buffer_size = cpal::BufferSize::Fixed(preferred);
                }
            }
        }

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if is_recording.load(Ordering::SeqCst) {
                        let mut samples = recorded_samples.lock().unwrap();
                        for &sample in data {
                            // Standard F32 to I16 conversion with saturation clamping
                            let s =
                                (sample * 32768.0).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                            samples.push(s);
                        }
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if is_recording.load(Ordering::SeqCst) {
                        let mut samples = recorded_samples.lock().unwrap();
                        samples.extend_from_slice(data);
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_input_stream(
                &stream_config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    if is_recording.load(Ordering::SeqCst) {
                        let mut samples = recorded_samples.lock().unwrap();
                        for &sample in data {
                            // Convert u16 to i16 (32768 is the center for u16)
                            let s = (sample as i32 - 32768) as i16;
                            samples.push(s);
                        }
                    }
                },
                err_fn,
                None,
            ),
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
                format!("Failed to build input stream for device: {}", e),
            )
        })?;

        stream.play().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to activate input stream: {}", e),
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
        self.stream = None; // Dropping the stream stops recording

        Ok(())
    }

    #[napi]
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    #[napi]
    pub fn get_buffer(&self) -> Result<SamplesBuffer> {
        let samples = self.recorded_samples.lock().unwrap().clone();
        Ok(SamplesBuffer::create(
            self.channels as u32,
            self.sample_rate,
            samples,
        ))
    }

    #[napi]
    pub fn clear(&mut self) {
        self.recorded_samples.lock().unwrap().clear();
    }
}

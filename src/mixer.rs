//! Audio mixer - blend multiple audio sources together

use crate::conversions::convert_channels_f32;
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::{OutputStreamBuilder, Sink, Source};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A mixer that combines multiple audio sources into a single output stream.
#[napi]
pub struct Mixer {
    sources: Arc<Mutex<Vec<MixerSource>>>,
    max_sources: usize,
    sample_rate: u32,
    channels: u16,
    volume: Arc<Mutex<f32>>,
    output_stream: Arc<Mutex<Option<rodio::OutputStream>>>,
    sink: Arc<Mutex<Option<Sink>>>,
    is_mixing: Arc<AtomicBool>,
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Mixer {
    fn drop(&mut self) {
        self.stop_mixing();
    }
}

#[napi]
impl Mixer {
    /// Create a new mixer with default settings (44100 Hz, stereo, max 16 sources).
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::with_config(44100, 2, 16)
    }

    /// Create a mixer with custom configuration.
    #[napi(factory)]
    pub fn with_config(sample_rate: u32, channels: u16, max_sources: u32) -> Self {
        Mixer {
            sources: Arc::new(Mutex::new(Vec::with_capacity(max_sources as usize))),
            max_sources: max_sources as usize,
            sample_rate,
            channels,
            volume: Arc::new(Mutex::new(1.0)),
            output_stream: Arc::new(Mutex::new(None)),
            sink: Arc::new(Mutex::new(None)),
            is_mixing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Add an audio source to the mixer.
    #[napi]
    pub fn add_source(&self, source: &MixerSource) -> Result<()> {
        let mut sources = self.sources.lock().unwrap_or_else(|e| e.into_inner());
        if sources.len() >= self.max_sources {
            return Err(Error::new(
                Status::GenericFailure,
                format!("Mixer at capacity (max {} sources)", self.max_sources),
            ));
        }
        sources.push(source.clone());
        Ok(())
    }

    /// Remove a source by its ID.
    #[napi]
    pub fn remove_source(&self, source_id: String) -> Result<()> {
        let mut sources = self.sources.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pos) = sources.iter().position(|s| s.id == source_id) {
            sources.remove(pos);
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "Source not found"))
        }
    }

    /// Get all current sources.
    #[napi]
    pub fn get_sources(&self) -> Vec<MixerSource> {
        self.sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Get the number of sources.
    #[napi]
    pub fn get_source_count(&self) -> u32 {
        self.sources.lock().unwrap_or_else(|e| e.into_inner()).len() as u32
    }

    /// Clear all sources.
    #[napi]
    pub fn clear(&self) {
        self.sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    /// Mix all sources at a specific time point.
    #[napi]
    pub fn sample_at(&self, time_ms: u32) -> Result<Vec<i16>> {
        let sources = self.sources.lock().unwrap_or_else(|e| e.into_inner());
        if sources.is_empty() {
            return Ok(Vec::new());
        }
        Ok(mix_frame(
            &sources,
            &self.volume,
            self.sample_rate,
            self.channels,
            time_ms as f64 / 1000.0,
        ))
    }

    /// Start mixing multiple sources in real time.
    #[napi]
    pub fn start_mixing(&self) -> Result<()> {
        if self.is_mixing.swap(true, Ordering::SeqCst) {
            return Err(Error::new(
                Status::GenericFailure,
                "Mixer is already running",
            ));
        }

        if self
            .sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
        {
            self.is_mixing.store(false, Ordering::SeqCst);
            return Err(Error::new(Status::InvalidArg, "No sources to mix"));
        }
        if self.sample_rate == 0 || self.channels == 0 {
            self.is_mixing.store(false, Ordering::SeqCst);
            return Err(Error::new(
                Status::InvalidArg,
                "Mixer sample rate and channels must be greater than zero",
            ));
        }

        let mut output_builder = match OutputStreamBuilder::from_default_device() {
            Ok(builder) => builder,
            Err(error) => {
                self.is_mixing.store(false, Ordering::SeqCst);
                return Err(Error::new(
                    Status::GenericFailure,
                    format!("Failed to create mixer output stream: {}", error),
                ));
            }
        };
        output_builder = output_builder
            .with_sample_rate(self.sample_rate)
            .with_channels(self.channels);
        let stream = match output_builder.open_stream() {
            Ok(stream) => stream,
            Err(error) => {
                self.is_mixing.store(false, Ordering::SeqCst);
                return Err(Error::new(
                    Status::GenericFailure,
                    format!("Failed to create mixer output stream: {}", error),
                ));
            }
        };
        let sink = Sink::connect_new(stream.mixer());
        let source = MixerOutput {
            sources: self.sources.clone(),
            volume: self.volume.clone(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            is_mixing: self.is_mixing.clone(),
            frame_index: 0,
            channel_index: self.channels as usize,
            current_frame: Vec::new(),
        };
        sink.append(source);
        sink.play();

        *self.output_stream.lock().unwrap_or_else(|e| e.into_inner()) = Some(stream);
        *self.sink.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink);
        Ok(())
    }

    /// Stop all mixing.
    #[napi]
    pub fn stop_mixing(&self) {
        self.is_mixing.store(false, Ordering::SeqCst);
        {
            let sink_guard = self.sink.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(sink) = sink_guard.as_ref() {
                sink.stop();
            }
        }
        *self.sink.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.output_stream.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Check whether a real-time mixer stream is running.
    #[napi]
    pub fn is_mixing(&self) -> bool {
        self.is_mixing.load(Ordering::SeqCst)
    }

    /// Get the sample rate of the mixer.
    #[napi]
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the channel count of the mixer.
    #[napi]
    pub fn get_channels(&self) -> u16 {
        self.channels
    }

    /// Set the master volume (0.0 to 1.0).
    #[napi]
    pub fn set_master_volume(&mut self, volume: f64) -> Result<()> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(Error::new(
                Status::InvalidArg,
                "Volume must be between 0.0 and 1.0",
            ));
        }
        *self.volume.lock().unwrap_or_else(|e| e.into_inner()) = volume as f32;
        Ok(())
    }

    /// Get the master volume.
    #[napi]
    pub fn get_master_volume(&self) -> f64 {
        *self.volume.lock().unwrap_or_else(|e| e.into_inner()) as f64
    }
}

/// A source that can be added to a mixer.
#[napi]
#[derive(Clone)]
pub struct MixerSource {
    id: String,
    samples: Vec<i16>,
    sample_rate: u32,
    channels: u16,
    volume: Arc<Mutex<f32>>,
    pan: Arc<Mutex<f32>>, // -1.0 (left) to 1.0 (right)
    enabled: Arc<Mutex<bool>>,
}

#[napi]
impl MixerSource {
    #[napi(constructor)]
    pub fn new(id: String, samples: Vec<i16>, sample_rate: u32, channels: u16) -> Self {
        Self {
            id,
            samples,
            sample_rate,
            channels,
            volume: Arc::new(Mutex::new(1.0)),
            pan: Arc::new(Mutex::new(0.0)),
            enabled: Arc::new(Mutex::new(true)),
        }
    }

    /// Get source ID.
    #[napi]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    /// Get audio samples.
    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        self.samples.clone()
    }

    /// Get one source frame at a specific time.
    #[napi]
    pub fn get_samples_at(&self, time_ms: u32) -> Result<Vec<i16>> {
        Ok(source_frame_at(self, time_ms as f64 / 1000.0)
            .unwrap_or_default()
            .into_iter()
            .map(sample_to_i16)
            .collect())
    }

    /// Get sample rate.
    #[napi]
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get channels.
    #[napi]
    pub fn get_channels(&self) -> u16 {
        self.channels
    }

    /// Set volume (0.0 to 1.0).
    #[napi]
    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(Error::new(
                Status::InvalidArg,
                "Volume must be between 0.0 and 1.0",
            ));
        }
        *self.volume.lock().unwrap_or_else(|e| e.into_inner()) = volume as f32;
        Ok(())
    }

    /// Get volume.
    #[napi]
    pub fn get_volume(&self) -> f64 {
        *self.volume.lock().unwrap_or_else(|e| e.into_inner()) as f64
    }

    /// Set pan (-1.0 left, 0.0 center, 1.0 right).
    #[napi]
    pub fn set_pan(&mut self, pan: f64) -> Result<()> {
        if !(-1.0..=1.0).contains(&pan) {
            return Err(Error::new(
                Status::InvalidArg,
                "Pan must be between -1.0 and 1.0",
            ));
        }
        *self.pan.lock().unwrap_or_else(|e| e.into_inner()) = pan as f32;
        Ok(())
    }

    /// Get pan.
    #[napi]
    pub fn get_pan(&self) -> f64 {
        *self.pan.lock().unwrap_or_else(|e| e.into_inner()) as f64
    }

    /// Enable or disable source.
    #[napi]
    pub fn set_enabled(&mut self, enabled: bool) {
        *self.enabled.lock().unwrap_or_else(|e| e.into_inner()) = enabled;
    }

    /// Check if source is enabled.
    #[napi]
    pub fn is_enabled(&self) -> bool {
        *self.enabled.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Get duration in milliseconds.
    #[napi]
    pub fn duration_ms(&self) -> u32 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0;
        }
        (self.samples.len() as f64 / (self.sample_rate as f64 * self.channels as f64) * 1000.0)
            as u32
    }
}

fn source_frame_at(source: &MixerSource, time_seconds: f64) -> Option<Vec<f32>> {
    let channels = source.channels as usize;
    if source.sample_rate == 0 || channels == 0 || !time_seconds.is_finite() || time_seconds < 0.0 {
        return None;
    }
    let frame_count = source.samples.len() / channels;
    if frame_count == 0 {
        return None;
    }

    let position = time_seconds * source.sample_rate as f64;
    if position >= frame_count as f64 {
        return None;
    }
    let index = position.floor() as usize;
    let next_index = (index + 1).min(frame_count - 1);
    let fraction = (position - index as f64) as f32;
    let mut frame = Vec::with_capacity(channels);
    for channel in 0..channels {
        let first = source.samples[index * channels + channel] as f32 / 32768.0;
        let second = source.samples[next_index * channels + channel] as f32 / 32768.0;
        frame.push(first + (second - first) * fraction);
    }
    Some(frame)
}

fn mix_frame(
    sources: &[MixerSource],
    master_volume: &Arc<Mutex<f32>>,
    sample_rate: u32,
    channels: u16,
    time_seconds: f64,
) -> Vec<i16> {
    let target_channels = channels as usize;
    if target_channels == 0 || sample_rate == 0 {
        return Vec::new();
    }

    let mut mixed = vec![0.0f32; target_channels];
    for source in sources {
        if !*source.enabled.lock().unwrap_or_else(|e| e.into_inner()) {
            continue;
        }
        let Some(frame) = source_frame_at(source, time_seconds) else {
            continue;
        };
        let converted = convert_channels_f32(&frame, source.channels, channels);
        let volume = *source.volume.lock().unwrap_or_else(|e| e.into_inner());
        let pan = *source.pan.lock().unwrap_or_else(|e| e.into_inner());
        for (channel, mixed_sample) in mixed.iter_mut().enumerate() {
            let mut gain = volume;
            if target_channels == 2 {
                gain *= if channel == 0 {
                    if pan > 0.0 {
                        1.0 - pan
                    } else {
                        1.0
                    }
                } else if pan < 0.0 {
                    1.0 + pan
                } else {
                    1.0
                };
            }
            *mixed_sample += converted.get(channel).copied().unwrap_or(0.0) * gain;
        }
    }

    let master = *master_volume.lock().unwrap_or_else(|e| e.into_inner());
    mixed
        .into_iter()
        .map(|sample| sample_to_i16(sample * master))
        .collect()
}

fn sample_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

struct MixerOutput {
    sources: Arc<Mutex<Vec<MixerSource>>>,
    volume: Arc<Mutex<f32>>,
    sample_rate: u32,
    channels: u16,
    is_mixing: Arc<AtomicBool>,
    frame_index: u64,
    channel_index: usize,
    current_frame: Vec<f32>,
}

impl Iterator for MixerOutput {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.is_mixing.load(Ordering::SeqCst) || self.channels == 0 {
            return None;
        }
        if self.channel_index >= self.channels as usize {
            let sources = self.sources.lock().unwrap_or_else(|e| e.into_inner());
            self.current_frame = mix_frame_f32(
                &sources,
                &self.volume,
                self.channels,
                self.frame_index as f64 / self.sample_rate.max(1) as f64,
            );
            self.frame_index = self.frame_index.saturating_add(1);
            self.channel_index = 0;
        }

        let sample = self
            .current_frame
            .get(self.channel_index)
            .copied()
            .unwrap_or(0.0);
        self.channel_index += 1;
        Some(sample)
    }
}

impl Source for MixerOutput {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

fn mix_frame_f32(
    sources: &[MixerSource],
    master_volume: &Arc<Mutex<f32>>,
    channels: u16,
    time_seconds: f64,
) -> Vec<f32> {
    let target_channels = channels as usize;
    let mut mixed = vec![0.0f32; target_channels];
    for source in sources {
        if !*source.enabled.lock().unwrap_or_else(|e| e.into_inner()) {
            continue;
        }
        let Some(frame) = source_frame_at(source, time_seconds) else {
            continue;
        };
        let converted = convert_channels_f32(&frame, source.channels, channels);
        let volume = *source.volume.lock().unwrap_or_else(|e| e.into_inner());
        let pan = *source.pan.lock().unwrap_or_else(|e| e.into_inner());
        for (channel, mixed_sample) in mixed.iter_mut().enumerate() {
            let mut gain = volume;
            if target_channels == 2 {
                gain *= if channel == 0 {
                    if pan > 0.0 {
                        1.0 - pan
                    } else {
                        1.0
                    }
                } else if pan < 0.0 {
                    1.0 + pan
                } else {
                    1.0
                };
            }
            *mixed_sample += converted.get(channel).copied().unwrap_or(0.0) * gain;
        }
    }
    let master = *master_volume.lock().unwrap_or_else(|e| e.into_inner());
    mixed
        .into_iter()
        .map(|sample| (sample * master).clamp(-1.0, 1.0))
        .collect()
}

/// Create a new mixer instance.
#[napi]
pub fn mixer(max_sources: Option<u32>) -> Mixer {
    Mixer::with_config(44100, 2, max_sources.unwrap_or(16))
}

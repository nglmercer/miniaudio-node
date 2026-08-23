//! Audio mixer - blend multiple audio sources together

use arc_swap::ArcSwap;
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::{OutputStreamBuilder, Sink, Source};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A mixer that combines multiple audio sources into a single output stream.
#[napi]
pub struct Mixer {
    // Writers publish a new immutable source snapshot. The realtime output
    // iterator loads the current snapshot without taking a mutex.
    sources: Arc<ArcSwap<Vec<MixerSource>>>,
    source_updates: Mutex<()>,
    max_sources: usize,
    sample_rate: u32,
    channels: u16,
    volume: Arc<AtomicU32>,
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
            sources: Arc::new(ArcSwap::from_pointee(Vec::with_capacity(
                max_sources as usize,
            ))),
            source_updates: Mutex::new(()),
            max_sources: max_sources as usize,
            sample_rate,
            channels,
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            output_stream: Arc::new(Mutex::new(None)),
            sink: Arc::new(Mutex::new(None)),
            is_mixing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Add an audio source to the mixer.
    #[napi]
    pub fn add_source(&self, source: &MixerSource) -> Result<()> {
        let _update_guard = self
            .source_updates
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut sources = self.sources.load_full().as_ref().clone();
        if sources.len() >= self.max_sources {
            return Err(Error::new(
                Status::GenericFailure,
                format!("Mixer at capacity (max {} sources)", self.max_sources),
            ));
        }
        sources.push(source.clone());
        self.sources.store(Arc::new(sources));
        Ok(())
    }

    /// Remove a source by its ID.
    #[napi]
    pub fn remove_source(&self, source_id: String) -> Result<()> {
        let _update_guard = self
            .source_updates
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut sources = self.sources.load_full().as_ref().clone();
        if let Some(pos) = sources.iter().position(|s| s.id == source_id) {
            sources.remove(pos);
            self.sources.store(Arc::new(sources));
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "Source not found"))
        }
    }

    /// Get all current sources.
    #[napi]
    pub fn get_sources(&self) -> Vec<MixerSource> {
        self.sources.load_full().as_ref().clone()
    }

    /// Get the number of sources.
    #[napi]
    pub fn get_source_count(&self) -> u32 {
        self.sources.load().len() as u32
    }

    /// Clear all sources.
    #[napi]
    pub fn clear(&self) {
        let _update_guard = self
            .source_updates
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.sources.store(Arc::new(Vec::new()));
    }

    /// Mix all sources at a specific time point.
    #[napi]
    pub fn sample_at(&self, time_ms: u32) -> Result<Vec<i16>> {
        let sources = self.sources.load();
        if sources.is_empty() {
            return Ok(Vec::new());
        }
        Ok(mix_frame(
            sources.as_ref(),
            load_float(&self.volume),
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

        if self.sources.load().is_empty() {
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
            // Allocate render scratch storage before the source is handed to
            // rodio. The iterator never resizes this buffer.
            current_frame: vec![0.0; self.channels as usize],
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
        self.volume
            .store((volume as f32).to_bits(), Ordering::Relaxed);
        Ok(())
    }

    /// Get the master volume.
    #[napi]
    pub fn get_master_volume(&self) -> f64 {
        load_float(&self.volume) as f64
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
    volume: Arc<AtomicU32>,
    pan: Arc<AtomicU32>, // -1.0 (left) to 1.0 (right)
    enabled: Arc<AtomicBool>,
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
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            pan: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            enabled: Arc::new(AtomicBool::new(true)),
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
        self.volume
            .store((volume as f32).to_bits(), Ordering::Relaxed);
        Ok(())
    }

    /// Get volume.
    #[napi]
    pub fn get_volume(&self) -> f64 {
        load_float(&self.volume) as f64
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
        self.pan.store((pan as f32).to_bits(), Ordering::Relaxed);
        Ok(())
    }

    /// Get pan.
    #[napi]
    pub fn get_pan(&self) -> f64 {
        load_float(&self.pan) as f64
    }

    /// Enable or disable source.
    #[napi]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Check if source is enabled.
    #[napi]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
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

fn source_frame_position(source: &MixerSource, time_seconds: f64) -> Option<(usize, usize, f32)> {
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
    Some((index, next_index, fraction))
}

fn interpolated_source_sample(
    source: &MixerSource,
    frame_index: usize,
    next_frame_index: usize,
    fraction: f32,
    channel: usize,
) -> f32 {
    let channels = source.channels as usize;
    let first = source.samples[frame_index * channels + channel] as f32 / 32768.0;
    let second = source.samples[next_frame_index * channels + channel] as f32 / 32768.0;
    first + (second - first) * fraction
}

fn source_frame_at(source: &MixerSource, time_seconds: f64) -> Option<Vec<f32>> {
    let (frame_index, next_frame_index, fraction) = source_frame_position(source, time_seconds)?;
    let channels = source.channels as usize;
    let mut frame = Vec::with_capacity(channels);
    for channel in 0..channels {
        frame.push(interpolated_source_sample(
            source,
            frame_index,
            next_frame_index,
            fraction,
            channel,
        ));
    }
    Some(frame)
}

/// Read one converted target-channel sample without creating an intermediate
/// source frame or channel-conversion vector. The realtime iterator uses this
/// to write directly into its preallocated frame buffer.
fn converted_source_sample(
    source: &MixerSource,
    frame_index: usize,
    next_frame_index: usize,
    fraction: f32,
    target_channel: usize,
    target_channels: usize,
) -> f32 {
    let source_channels = source.channels as usize;
    if source_channels == target_channels {
        return interpolated_source_sample(
            source,
            frame_index,
            next_frame_index,
            fraction,
            target_channel,
        );
    }

    let source_average = || {
        (0..source_channels)
            .map(|channel| {
                interpolated_source_sample(source, frame_index, next_frame_index, fraction, channel)
            })
            .sum::<f32>()
            / source_channels as f32
    };

    if target_channels == 1 {
        return source_average();
    }
    if source_channels == 1 {
        return interpolated_source_sample(source, frame_index, next_frame_index, fraction, 0);
    }
    if target_channels < source_channels {
        let mut sum = 0.0;
        let mut count = 0usize;
        for source_channel in (target_channel..source_channels).step_by(target_channels) {
            sum += interpolated_source_sample(
                source,
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
        interpolated_source_sample(
            source,
            frame_index,
            next_frame_index,
            fraction,
            target_channel,
        )
    } else {
        source_average()
    }
}

fn mix_frame(
    sources: &[MixerSource],
    master_volume: f32,
    sample_rate: u32,
    channels: u16,
    time_seconds: f64,
) -> Vec<i16> {
    let target_channels = channels as usize;
    if target_channels == 0 || sample_rate == 0 {
        return Vec::new();
    }

    let mut mixed = vec![0.0f32; target_channels];
    mix_frame_f32_into(sources, 1.0, channels, time_seconds, &mut mixed);

    mixed
        .into_iter()
        .map(|sample| sample_to_i16(sample * master_volume))
        .collect()
}

fn sample_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

struct MixerOutput {
    sources: Arc<ArcSwap<Vec<MixerSource>>>,
    volume: Arc<AtomicU32>,
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
            let sources = self.sources.load();
            mix_frame_f32_into(
                sources.as_ref(),
                load_float(&self.volume),
                self.channels,
                self.frame_index as f64 / self.sample_rate.max(1) as f64,
                &mut self.current_frame,
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

fn mix_frame_f32_into(
    sources: &[MixerSource],
    master_volume: f32,
    channels: u16,
    time_seconds: f64,
    mixed: &mut [f32],
) {
    let target_channels = channels as usize;
    if target_channels == 0 || mixed.len() < target_channels {
        return;
    }
    mixed[..target_channels].fill(0.0);
    for source in sources {
        if !source.enabled.load(Ordering::Relaxed) {
            continue;
        }
        let Some((frame_index, next_frame_index, fraction)) =
            source_frame_position(source, time_seconds)
        else {
            continue;
        };
        let volume = load_float(&source.volume);
        let pan = load_float(&source.pan);
        for (channel, mixed_sample) in mixed.iter_mut().enumerate().take(target_channels) {
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
            *mixed_sample += converted_source_sample(
                source,
                frame_index,
                next_frame_index,
                fraction,
                channel,
                target_channels,
            ) * gain;
        }
    }
    for sample in &mut mixed[..target_channels] {
        *sample = (*sample * master_volume).clamp(-1.0, 1.0);
    }
}

fn load_float(value: &AtomicU32) -> f32 {
    f32::from_bits(value.load(Ordering::Relaxed))
}

/// Create a new mixer instance.
#[napi]
pub fn mixer(max_sources: Option<u32>) -> Mixer {
    Mixer::with_config(44100, 2, max_sources.unwrap_or(16))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realtime_iterator_reuses_preallocated_frame_storage() {
        let source = MixerSource::new("source".to_string(), vec![1_000, -1_000], 44_100, 2);
        let sources = Arc::new(ArcSwap::from_pointee(vec![source]));
        let is_mixing = Arc::new(AtomicBool::new(true));
        let mut output = MixerOutput {
            sources,
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            sample_rate: 44_100,
            channels: 2,
            is_mixing,
            frame_index: 0,
            channel_index: 2,
            current_frame: vec![0.0; 2],
        };
        let frame_pointer = output.current_frame.as_ptr();

        for _ in 0..1_000 {
            assert!(output.next().is_some());
            assert_eq!(output.current_frame.as_ptr(), frame_pointer);
            assert_eq!(output.current_frame.capacity(), 2);
        }
    }
}

//! Audio mixer - blend multiple audio sources together

use napi::{Error, Result, Status};
use napi_derive::napi;
use std::sync::{Arc, Mutex};

/// A mixer that combines multiple audio sources into a single output stream
#[napi]
pub struct Mixer {
    sources: Arc<Mutex<Vec<MixerSource>>>,
    max_sources: usize,
    sample_rate: u32,
    channels: u16,
    volume: Arc<Mutex<f32>>,
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl Mixer {
    /// Create a new mixer with default settings (44100 Hz, stereo, max 16 sources)
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::with_config(44100, 2, 16)
    }

    /// Create a mixer with custom configuration
    #[napi(factory)]
    pub fn with_config(sample_rate: u32, channels: u16, max_sources: u32) -> Self {
        Mixer {
            sources: Arc::new(Mutex::new(Vec::with_capacity(max_sources as usize))),
            max_sources: max_sources as usize,
            sample_rate,
            channels,
            volume: Arc::new(Mutex::new(1.0)),
        }
    }

    /// Add an audio source to the mixer
    #[napi]
    pub fn add_source(&self, source: &MixerSource) -> Result<()> {
        let mut sources = self.sources.lock().unwrap();
        if sources.len() >= self.max_sources {
            return Err(Error::new(
                Status::GenericFailure,
                format!("Mixer at capacity (max {} sources)", self.max_sources),
            ));
        }
        sources.push(source.clone());
        Ok(())
    }

    /// Remove a source by its ID
    #[napi]
    pub fn remove_source(&self, source_id: String) -> Result<()> {
        let mut sources = self.sources.lock().unwrap();
        if let Some(pos) = sources.iter().position(|s| s.id == source_id) {
            sources.remove(pos);
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "Source not found"))
        }
    }

    /// Get all current sources
    #[napi]
    pub fn get_sources(&self) -> Vec<MixerSource> {
        self.sources.lock().unwrap().to_vec()
    }

    /// Get the number of sources
    #[napi]
    pub fn get_source_count(&self) -> u32 {
        self.sources.lock().unwrap().len() as u32
    }

    /// Clear all sources
    #[napi]
    pub fn clear(&self) {
        self.sources.lock().unwrap().clear();
    }

    /// Mix all sources at a specific time point (synchronous operation)
    /// Returns a buffer of mixed samples
    #[napi]
    pub fn sample_at(&self, time_ms: u32) -> Result<Vec<i16>> {
        let sources = self.sources.lock().unwrap();
        if sources.is_empty() {
            return Ok(vec![]);
        }

        let mut mixed: Vec<i32> = vec![0; self.channels as usize];
        let mut count = 0;

        for source in sources.iter() {
            if let Ok(samples) = source.get_samples_at(time_ms) {
                for (i, &sample) in samples.iter().enumerate() {
                    if i < mixed.len() {
                        mixed[i] += sample as i32;
                    }
                }
                count += 1;
            }
        }

        if count == 0 {
            return Ok(vec![0; self.channels as usize]);
        }

        // Normalize to prevent clipping
        let output: Vec<i16> = mixed
            .iter()
            .map(|&s| (s / count).clamp(-32768, 32767) as i16)
            .collect();

        Ok(output)
    }

    /// Start mixing multiple sources in real-time (simulated)
    #[napi]
    pub fn start_mixing(&self) -> Result<()> {
        // In a real implementation, this would start an audio thread
        // For now, we just verify we have something to mix
        let sources = self.sources.lock().unwrap();
        if sources.is_empty() {
            return Err(Error::new(Status::InvalidArg, "No sources to mix"));
        }
        Ok(())
    }

    /// Stop all mixing
    #[napi]
    pub fn stop_mixing(&self) {}

    /// Get the sample rate of the mixer
    #[napi]
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the channel count of the mixer
    #[napi]
    pub fn get_channels(&self) -> u16 {
        self.channels
    }

    /// Set the master volume (0.0 to 1.0)
    #[napi]
    pub fn set_master_volume(&mut self, volume: f64) -> Result<()> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(Error::new(
                Status::InvalidArg,
                "Volume must be between 0.0 and 1.0",
            ));
        }
        *self.volume.lock().unwrap() = volume as f32;
        Ok(())
    }

    /// Get the master volume
    #[napi]
    pub fn get_master_volume(&self) -> f64 {
        *self.volume.lock().unwrap() as f64
    }
}

/// A source that can be added to a mixer
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

    /// Get source ID
    #[napi]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    /// Get audio samples
    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        self.samples.clone()
    }

    /// Get samples at a specific time (simplified to return relative audio)
    #[napi]
    pub fn get_samples_at(&self, time_ms: u32) -> Result<Vec<i16>> {
        // Calculate sample offset based on time and sample rate
        let time_sec = time_ms as f64 / 1000.0;
        let sample_offset = (time_sec * self.sample_rate as f64 * self.channels as f64) as usize;

        if sample_offset >= self.samples.len() {
            return Ok(vec![]);
        }

        // Return samples starting from the calculated offset
        let len = (sample_offset + self.channels as usize).min(self.samples.len());
        Ok(self.samples[sample_offset..len].to_vec())
    }

    /// Get sample rate
    #[napi]
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get channels
    #[napi]
    pub fn get_channels(&self) -> u16 {
        self.channels
    }

    /// Set volume (0.0 to 1.0)
    #[napi]
    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(Error::new(
                Status::InvalidArg,
                "Volume must be between 0.0 and 1.0",
            ));
        }
        *self.volume.lock().unwrap() = volume as f32;
        Ok(())
    }

    /// Get volume
    #[napi]
    pub fn get_volume(&self) -> f64 {
        *self.volume.lock().unwrap() as f64
    }

    /// Set pan (-1.0 left, 0.0 center, 1.0 right)
    #[napi]
    pub fn set_pan(&mut self, pan: f64) -> Result<()> {
        if !(-1.0..=1.0).contains(&pan) {
            return Err(Error::new(
                Status::InvalidArg,
                "Pan must be between -1.0 and 1.0",
            ));
        }
        *self.pan.lock().unwrap() = pan as f32;
        Ok(())
    }

    /// Get pan
    #[napi]
    pub fn get_pan(&self) -> f64 {
        *self.pan.lock().unwrap() as f64
    }

    /// Enable or disable source
    #[napi]
    pub fn set_enabled(&mut self, enabled: bool) {
        *self.enabled.lock().unwrap() = enabled;
    }

    /// Check if source is enabled
    #[napi]
    pub fn is_enabled(&self) -> bool {
        *self.enabled.lock().unwrap()
    }

    /// Get duration in milliseconds
    #[napi]
    pub fn duration_ms(&self) -> u32 {
        (self.samples.len() as f64 / (self.sample_rate as f64 * self.channels as f64) * 1000.0)
            as u32
    }
}

/// Create a new mixer instance
#[napi]
pub fn mixer(max_sources: Option<u32>) -> Mixer {
    Mixer::with_config(44100, 2, max_sources.unwrap_or(16))
}

//! Audio decoder for various audio formats

use crate::conversions::{convert_channels_f32, resample_f32};
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::{Decoder, Source};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::sync::{Arc, Mutex};

/// Decoder for audio files in various formats (WAV, MP3, FLAC, OGG, etc.)
#[napi]
pub struct AudioDecoder {
    data: Arc<Mutex<Option<Vec<u8>>>>,
    file_path: Option<String>,
    source_sample_rate: u32,
    source_channels: u16,
    sample_rate: u32,
    channels: u16,
    duration: f64,
    target_sample_rate: Option<u32>,
    target_channels: Option<u16>,
}

#[napi]
impl AudioDecoder {
    /// Create a decoder from a file path
    #[napi(constructor)]
    pub fn from_file(file_path: String) -> Result<Self> {
        Self::from_file_with_config(file_path, None, None)
    }

    fn from_file_with_config(
        file_path: String,
        target_sample_rate: Option<u32>,
        target_channels: Option<u16>,
    ) -> Result<Self> {
        let path = std::path::Path::new(&file_path);
        if !path.exists() {
            return Err(Error::new(
                Status::InvalidArg,
                format!("File not found: {}", file_path),
            ));
        }

        let file = File::open(path)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to open file: {}", e)))?;

        let reader = BufReader::new(file);
        let source = Decoder::new(reader).map_err(|e| {
            Error::new(Status::InvalidArg, format!("Failed to decode audio: {}", e))
        })?;

        let source_sample_rate = source.sample_rate();
        let source_channels = source.channels();
        let duration = source.total_duration().map_or(0.0, |d| d.as_secs_f64());
        let sample_rate = target_sample_rate.unwrap_or(source_sample_rate);
        let channels = target_channels.unwrap_or(source_channels);

        Ok(Self {
            data: Arc::new(Mutex::new(None)),
            file_path: Some(file_path),
            source_sample_rate,
            source_channels,
            sample_rate,
            channels,
            duration,
            target_sample_rate,
            target_channels,
        })
    }

    /// Create a decoder from raw audio data
    #[napi(factory)]
    pub fn from_data(data: Vec<u8>) -> Result<Self> {
        Self::from_data_with_config(data, None, None)
    }

    fn from_data_with_config(
        data: Vec<u8>,
        target_sample_rate: Option<u32>,
        target_channels: Option<u16>,
    ) -> Result<Self> {
        let cursor = Cursor::new(data.clone());
        let source = Decoder::new(cursor).map_err(|e| {
            Error::new(Status::InvalidArg, format!("Failed to decode audio: {}", e))
        })?;

        let source_sample_rate = source.sample_rate();
        let source_channels = source.channels();
        let duration = source.total_duration().map_or(0.0, |d| d.as_secs_f64());
        let sample_rate = target_sample_rate.unwrap_or(source_sample_rate);
        let channels = target_channels.unwrap_or(source_channels);

        Ok(Self {
            data: Arc::new(Mutex::new(Some(data))),
            file_path: None,
            source_sample_rate,
            source_channels,
            sample_rate,
            channels,
            duration,
            target_sample_rate,
            target_channels,
        })
    }

    /// Get sample rate of decoded audio
    #[napi]
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get number of channels (1=mono, 2=stereo, etc.)
    #[napi]
    pub fn get_channels(&self) -> u16 {
        self.channels
    }

    /// Get duration in seconds
    #[napi]
    pub fn get_duration(&self) -> f64 {
        self.duration
    }

    /// Reset decoder to beginning
    #[napi]
    pub fn reset(&mut self) -> Result<()> {
        match &self.file_path {
            Some(path) => {
                let path_clone = path.clone();
                *self = AudioDecoder::from_file_with_config(
                    path_clone,
                    self.target_sample_rate,
                    self.target_channels,
                )?;
                Ok(())
            }
            None => {
                let data = {
                    let data_guard = self.data.lock().unwrap_or_else(|e| e.into_inner());
                    data_guard.as_ref().cloned()
                };
                if let Some(data) = data {
                    *self = AudioDecoder::from_data_with_config(
                        data,
                        self.target_sample_rate,
                        self.target_channels,
                    )?;
                    Ok(())
                } else {
                    Err(Error::new(Status::InvalidArg, "No audio data to reset"))
                }
            }
        }
    }

    /// Decode all audio samples into a vector
    #[napi]
    pub fn decode_to_samples(&self) -> Result<Vec<i16>> {
        let source_samples = if let Some(file_path) = &self.file_path {
            let file = File::open(file_path).map_err(|e| {
                Error::new(Status::InvalidArg, format!("Failed to open file: {}", e))
            })?;
            let reader = BufReader::new(file);
            let source = Decoder::new(reader).map_err(|e| {
                Error::new(Status::InvalidArg, format!("Failed to decode audio: {}", e))
            })?;
            Ok(source.collect::<Vec<f32>>())
        } else {
            let data_guard = self.data.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(data) = data_guard.as_ref() {
                let cursor = Cursor::new(data.clone());
                let source = Decoder::new(cursor).map_err(|e| {
                    Error::new(Status::InvalidArg, format!("Failed to decode audio: {}", e))
                })?;
                Ok(source.collect::<Vec<f32>>())
            } else {
                Err(Error::new(Status::InvalidArg, "No audio data to decode"))
            }
        }?;

        let channel_converted =
            convert_channels_f32(&source_samples, self.source_channels, self.channels);
        let converted = resample_f32(
            &channel_converted,
            self.source_sample_rate,
            self.sample_rate,
            self.channels,
        );
        Ok(converted
            .into_iter()
            .map(|sample| sample.clamp(-1.0, 1.0) * 32767.0)
            .map(|sample| sample.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16)
            .collect())
    }

    /// Get a slice of decoded samples (limited by duration to prevent memory issues)
    #[napi]
    pub fn decode_slice(&self, start_seconds: f64, end_seconds: f64) -> Result<Vec<i16>> {
        if !start_seconds.is_finite()
            || !end_seconds.is_finite()
            || start_seconds < 0.0
            || end_seconds < 0.0
            || start_seconds >= end_seconds
        {
            return Err(Error::new(
                Status::InvalidArg,
                "Slice bounds must be finite, non-negative, and ordered",
            ));
        }
        let mut samples = self.decode_to_samples()?;
        let start_idx = (start_seconds * self.sample_rate as f64 * self.channels as f64) as usize;
        let end_idx = (end_seconds * self.sample_rate as f64 * self.channels as f64) as usize;
        let end_idx = end_idx.min(samples.len());

        if start_idx >= samples.len() {
            return Ok(Vec::new());
        }

        if start_idx >= end_idx {
            return Ok(Vec::new());
        }

        samples.drain(end_idx..);
        samples.drain(..start_idx);
        Ok(samples)
    }

    /// Check if this is a stereo file
    #[napi]
    pub fn is_stereo(&self) -> bool {
        self.channels == 2
    }

    /// Check if this is a mono file
    #[napi]
    pub fn is_mono(&self) -> bool {
        self.channels == 1
    }
}

/// Looped decoder - decodes audio and repeats it indefinitely
#[napi]
pub struct LoopedDecoder {
    decoder: AudioDecoder,
    loop_count: u32,
}

#[napi]
impl LoopedDecoder {
    /// Create a new looped decoder
    #[napi(constructor)]
    pub fn new(decoder: &AudioDecoder, loop_count: Option<u32>) -> Result<Self> {
        let decoder_clone = AudioDecoder {
            data: decoder.data.clone(),
            file_path: decoder.file_path.clone(),
            source_sample_rate: decoder.source_sample_rate,
            source_channels: decoder.source_channels,
            sample_rate: decoder.sample_rate,
            channels: decoder.channels,
            duration: decoder.duration,
            target_sample_rate: decoder.target_sample_rate,
            target_channels: decoder.target_channels,
        };
        Ok(Self {
            decoder: decoder_clone,
            loop_count: match loop_count.unwrap_or(u32::MAX) {
                0 => u32::MAX,
                count => count,
            },
        })
    }

    /// Get the loop count (0 = infinite)
    #[napi]
    pub fn get_loop_count(&self) -> u32 {
        if self.loop_count == u32::MAX {
            0
        } else {
            self.loop_count
        }
    }

    /// Set the loop count (use u32::MAX for infinite)
    #[napi]
    pub fn set_loop_count(&mut self, count: u32) {
        self.loop_count = if count == 0 { u32::MAX } else { count };
    }

    /// Decode with loops applied. Infinite looping cannot be materialized into
    /// a finite vector and returns an error.
    #[napi]
    pub fn decode_looped(&self) -> Result<Vec<i16>> {
        let samples = self.decoder.decode_to_samples()?;
        if samples.is_empty() || self.loop_count == 1 {
            return Ok(samples);
        }

        if self.loop_count == u32::MAX {
            return Err(Error::new(
                Status::InvalidArg,
                "Infinite looping cannot be materialized as a finite sample array",
            ));
        }
        let loop_count = self.loop_count;

        let mut result = Vec::with_capacity(samples.len() * loop_count as usize);
        for _ in 0..loop_count {
            result.extend_from_slice(&samples);
        }

        Ok(result)
    }

    /// Get reference to inner decoder
    #[napi]
    pub fn get_decoder(&self) -> AudioDecoder {
        // Return a copy of the decoder
        if let Some(path) = &self.decoder.file_path {
            AudioDecoder::from_file_with_config(
                path.clone(),
                self.decoder.target_sample_rate,
                self.decoder.target_channels,
            )
            .unwrap_or_else(|_| AudioDecoder {
                data: Arc::new(Mutex::new(None)),
                file_path: None,
                source_sample_rate: 44100,
                source_channels: 2,
                sample_rate: 44100,
                channels: 2,
                duration: 0.0,
                target_sample_rate: None,
                target_channels: None,
            })
        } else {
            let data_guard = self.decoder.data.lock().unwrap_or_else(|e| e.into_inner());
            let data = data_guard.as_ref().cloned().unwrap_or_default();
            AudioDecoder::from_data_with_config(
                data,
                self.decoder.target_sample_rate,
                self.decoder.target_channels,
            )
            .unwrap_or_else(|_| AudioDecoder {
                data: Arc::new(Mutex::new(None)),
                file_path: None,
                source_sample_rate: 44100,
                source_channels: 2,
                sample_rate: 44100,
                channels: 2,
                duration: 0.0,
                target_sample_rate: None,
                target_channels: None,
            })
        }
    }
}

/// Decoder builder for configuring decoder behavior
#[napi]
pub struct DecoderBuilder {
    enable_looping: bool,
    loop_count: u32,
    sample_rate: Option<u32>,
    channels: Option<u16>,
}

impl Default for DecoderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl DecoderBuilder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            enable_looping: false,
            loop_count: 1,
            sample_rate: None,
            channels: None,
        }
    }

    #[napi]
    pub fn set_loop_enabled(&mut self, enabled: bool) {
        self.enable_looping = enabled;
    }

    #[napi]
    pub fn set_loop_count(&mut self, count: u32) {
        self.loop_count = if count == 0 { u32::MAX } else { count };
    }

    #[napi]
    pub fn set_sample_rate(&mut self, rate: u32) {
        self.sample_rate = Some(rate);
    }

    #[napi]
    pub fn set_channels(&mut self, channels: u16) {
        self.channels = Some(channels);
    }

    fn validate_format(&self) -> Result<()> {
        if self.sample_rate == Some(0) || self.channels == Some(0) {
            return Err(Error::new(
                Status::InvalidArg,
                "Sample rate and channel count must be greater than zero",
            ));
        }
        Ok(())
    }

    #[napi]
    pub fn build_from_file(&self, file_path: String) -> Result<AudioDecoder> {
        self.validate_format()?;
        AudioDecoder::from_file_with_config(file_path, self.sample_rate, self.channels)
    }

    #[napi]
    pub fn build_from_data(&self, data: Vec<u8>) -> Result<AudioDecoder> {
        self.validate_format()?;
        AudioDecoder::from_data_with_config(data, self.sample_rate, self.channels)
    }

    #[napi]
    pub fn build_looped(&self, file_path: String) -> Result<LoopedDecoder> {
        self.validate_format()?;
        let decoder =
            AudioDecoder::from_file_with_config(file_path, self.sample_rate, self.channels)?;
        let loop_count = if self.enable_looping {
            self.loop_count
        } else {
            1
        };
        LoopedDecoder::new(&decoder, Some(loop_count))
    }
}

/// Decoder builder settings
#[napi]
pub struct DecoderBuilderSettings {
    pub enable_looping: bool,
    pub loop_count: u32,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

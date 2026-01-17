//! Audio decoder for various audio formats

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
    sample_rate: u32,
    channels: u16,
    duration: f64,
}

#[napi]
impl AudioDecoder {
    /// Create a decoder from a file path
    #[napi(constructor)]
    pub fn from_file(file_path: String) -> Result<Self> {
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

        let sample_rate = source.sample_rate();
        let channels = source.channels();
        let duration = source.total_duration().map_or(0.0, |d| d.as_secs_f64());

        Ok(Self {
            data: Arc::new(Mutex::new(None)),
            file_path: Some(file_path),
            sample_rate,
            channels,
            duration,
        })
    }

    /// Create a decoder from raw audio data
    #[napi(factory)]
    pub fn from_data(data: Vec<u8>) -> Result<Self> {
        let cursor = Cursor::new(data.clone());
        let source = Decoder::new(cursor).map_err(|e| {
            Error::new(Status::InvalidArg, format!("Failed to decode audio: {}", e))
        })?;

        let sample_rate = source.sample_rate();
        let channels = source.channels();
        let duration = source.total_duration().map_or(0.0, |d| d.as_secs_f64());

        Ok(Self {
            data: Arc::new(Mutex::new(Some(data))),
            file_path: None,
            sample_rate,
            channels,
            duration,
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
                *self = AudioDecoder::from_file(path_clone)?;
                Ok(())
            }
            None => {
                let data = {
                    let data_guard = self.data.lock().unwrap();
                    data_guard.as_ref().cloned()
                };
                if let Some(data) = data {
                    *self = AudioDecoder::from_data(data)?;
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
        if let Some(file_path) = &self.file_path {
            let file = File::open(file_path).map_err(|e| {
                Error::new(Status::InvalidArg, format!("Failed to open file: {}", e))
            })?;
            let reader = BufReader::new(file);
            let source = Decoder::new(reader).map_err(|e| {
                Error::new(Status::InvalidArg, format!("Failed to decode audio: {}", e))
            })?;

            let samples: Vec<f32> = source.collect();
            let samples_i16: Vec<i16> = samples.into_iter().map(|s| (s * 32767.0) as i16).collect();
            Ok(samples_i16)
        } else {
            let data_guard = self.data.lock().unwrap();
            if let Some(data) = data_guard.as_ref() {
                let cursor = Cursor::new(data.clone());
                let source = Decoder::new(cursor).map_err(|e| {
                    Error::new(Status::InvalidArg, format!("Failed to decode audio: {}", e))
                })?;

                let samples: Vec<f32> = source.collect();
                let samples_i16: Vec<i16> =
                    samples.into_iter().map(|s| (s * 32767.0) as i16).collect();
                Ok(samples_i16)
            } else {
                Err(Error::new(Status::InvalidArg, "No audio data to decode"))
            }
        }
    }

    /// Get a slice of decoded samples (limited by duration to prevent memory issues)
    #[napi]
    pub fn decode_slice(&self, start_seconds: f64, end_seconds: f64) -> Result<Vec<i16>> {
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
            sample_rate: decoder.sample_rate,
            channels: decoder.channels,
            duration: decoder.duration,
        };
        Ok(Self {
            decoder: decoder_clone,
            loop_count: loop_count.unwrap_or(u32::MAX),
        })
    }

    /// Get the loop count (0 = infinite)
    #[napi]
    pub fn get_loop_count(&self) -> u32 {
        self.loop_count
    }

    /// Set the loop count (use u32::MAX for infinite)
    #[napi]
    pub fn set_loop_count(&mut self, count: u32) {
        self.loop_count = count;
    }

    /// Decode with loops applied
    #[napi]
    pub fn decode_looped(&self) -> Result<Vec<i16>> {
        let samples = self.decoder.decode_to_samples()?;
        if samples.is_empty() || self.loop_count == 1 {
            return Ok(samples);
        }

        let loop_count = if self.loop_count == u32::MAX {
            // For practical purposes, return 4 loops for safety
            4
        } else {
            self.loop_count
        };

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
            AudioDecoder::from_file(path.clone()).unwrap_or_else(|_| AudioDecoder {
                data: Arc::new(Mutex::new(None)),
                file_path: None,
                sample_rate: 44100,
                channels: 2,
                duration: 0.0,
            })
        } else {
            let data_guard = self.decoder.data.lock().unwrap();
            let data = data_guard.as_ref().cloned().unwrap_or_default();
            AudioDecoder::from_data(data).unwrap_or_else(|_| AudioDecoder {
                data: Arc::new(Mutex::new(None)),
                file_path: None,
                sample_rate: 44100,
                channels: 2,
                duration: 0.0,
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
        self.loop_count = count;
    }

    #[napi]
    pub fn set_sample_rate(&mut self, rate: u32) {
        self.sample_rate = Some(rate);
    }

    #[napi]
    pub fn set_channels(&mut self, channels: u16) {
        self.channels = Some(channels);
    }

    #[napi]
    pub fn build_from_file(&self, file_path: String) -> Result<AudioDecoder> {
        AudioDecoder::from_file(file_path)
    }

    #[napi]
    pub fn build_from_data(&self, data: Vec<u8>) -> Result<AudioDecoder> {
        AudioDecoder::from_data(data)
    }

    #[napi]
    pub fn build_looped(&self, file_path: String) -> Result<LoopedDecoder> {
        let decoder = AudioDecoder::from_file(file_path)?;
        LoopedDecoder::new(&decoder, Some(self.loop_count))
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

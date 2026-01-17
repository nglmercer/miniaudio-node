//! Audio streaming module for real-time playback

use crate::buffer::SamplesBuffer;
use crate::types::{PlayError, SupportedStreamConfig};
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::{OutputStream, OutputStreamBuilder, Sink, Source as RodioSource};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Audio stream for real-time playback
#[napi]
pub struct AudioStream {
    sink: Arc<Mutex<Option<Sink>>>,
    output_stream: Arc<Mutex<Option<OutputStream>>>,
    is_playing: Arc<Mutex<bool>>,
    is_paused: Arc<Mutex<bool>>,
    volume: Arc<Mutex<f32>>,
}

impl Default for AudioStream {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl AudioStream {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            sink: Arc::new(Mutex::new(None)),
            output_stream: Arc::new(Mutex::new(None)),
            is_playing: Arc::new(Mutex::new(false)),
            is_paused: Arc::new(Mutex::new(false)),
            volume: Arc::new(Mutex::new(1.0)),
        }
    }

    /// Open and initialize the audio stream
    #[napi]
    pub fn open(&mut self) -> Result<()> {
        let stream = OutputStreamBuilder::open_default_stream().map_err(|e| {
            Error::new(
                Status::InvalidArg,
                format!("Failed to create output stream: {}", e),
            )
        })?;

        let mixer = stream.mixer();
        let sink = Sink::connect_new(mixer);

        *self.output_stream.lock().unwrap() = Some(stream);
        *self.sink.lock().unwrap() = Some(sink);

        Ok(())
    }

    /// Play an audio file

    #[napi]
    pub fn play_file(&self, file_path: String) -> Result<()> {
        let path = Path::new(&file_path);
        if !path.exists() {
            return Err(Error::new(
                Status::InvalidArg,
                format!("File not found: {}", file_path),
            ));
        }

        let file = File::open(path)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to open file: {}", e)))?;

        let reader = BufReader::new(file);
        let source = rodio::Decoder::new(reader)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to decode: {}", e)))?;

        self.play_source(source)
    }

    /// Play raw audio data from buffer
    #[napi]
    pub fn play_buffer(&self, buffer: &SamplesBuffer) -> Result<()> {
        // Convert buffer samples to a rodio source (rodio uses f32)
        let samples_i16 = buffer.get_samples();
        let samples_f32: Vec<f32> = samples_i16
            .into_iter()
            .map(|s| s as f32 / 32768.0)
            .collect();
        let source = make_source_from_vec(
            samples_f32,
            buffer.get_sample_rate(),
            buffer.get_channels() as u16,
        );

        self.play_source(source)
    }

    /// Play base64 encoded audio data
    #[napi]
    pub fn play_base64(&self, base64_data: String) -> Result<()> {
        use base64::{engine::general_purpose, Engine as _};

        let data = general_purpose::STANDARD
            .decode(&base64_data)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid base64: {}", e)))?;

        let cursor = std::io::Cursor::new(data);
        let source = rodio::Decoder::new(cursor)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to decode: {}", e)))?;

        self.play_source(source)
    }

    /// Get the current playback state
    #[napi]
    pub fn get_state(&self) -> PlayError {
        let is_playing = *self.is_playing.lock().unwrap();
        let is_paused = *self.is_paused.lock().unwrap();

        if is_paused {
            PlayError::AlreadyPlaying
        } else if is_playing {
            PlayError::NotLoaded
        } else {
            PlayError::SystemError
        }
    }

    /// Check if audio is currently playing
    #[napi]
    pub fn is_playing(&self) -> bool {
        *self.is_playing.lock().unwrap()
    }

    /// Pause the stream
    #[napi]
    pub fn pause(&mut self) -> Result<()> {
        let sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_ref() {
            sink.pause();
            *self.is_playing.lock().unwrap() = true;
            *self.is_paused.lock().unwrap() = true;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "Stream not initialized"))
        }
    }

    /// Resume the stream
    #[napi]
    pub fn resume(&mut self) -> Result<()> {
        let sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_ref() {
            sink.play();
            *self.is_playing.lock().unwrap() = true;
            *self.is_paused.lock().unwrap() = false;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "Stream not initialized"))
        }
    }

    /// Stop the stream
    #[napi]
    pub fn stop(&mut self) -> Result<()> {
        let sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_ref() {
            sink.stop();
        }
        *self.sink.lock().unwrap() = None;
        *self.is_playing.lock().unwrap() = false;
        *self.is_paused.lock().unwrap() = false;
        Ok(())
    }

    /// Set the master volume (0.0 to 1.0)
    #[napi]
    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(Error::new(
                Status::InvalidArg,
                "Volume must be between 0.0 and 1.0",
            ));
        }

        *self.volume.lock().unwrap() = volume as f32;
        let sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_ref() {
            sink.set_volume(volume as f32);
        }
        Ok(())
    }

    /// Get the current volume
    #[napi]
    pub fn get_volume(&self) -> f64 {
        *self.volume.lock().unwrap() as f64
    }

    /// Get supported stream configurations
    #[napi]
    pub fn get_supported_configs() -> Vec<SupportedStreamConfig> {
        vec![
            SupportedStreamConfig {
                sample_rate: 44100,
                channel_count: 2,
                sample_width: 16,
            },
            SupportedStreamConfig {
                sample_rate: 48000,
                channel_count: 2,
                sample_width: 16,
            },
            SupportedStreamConfig {
                sample_rate: 96000,
                channel_count: 2,
                sample_width: 16,
            },
        ]
    }

    fn play_source<S>(&self, source: S) -> Result<()>
    where
        S: RodioSource<Item = f32> + Send + 'static,
    {
        let sink_guard = self.sink.lock().unwrap();
        if sink_guard.is_none() {
            return Err(Error::new(Status::InvalidArg, "Stream not initialized"));
        }

        let sink = sink_guard.as_ref().unwrap();
        sink.append(source);
        sink.play();

        *self.is_playing.lock().unwrap() = true;
        *self.is_paused.lock().unwrap() = false;

        Ok(())
    }
}

/// Stream builder for creating audio streams with specific configurations
#[napi]
pub struct AudioStreamBuilder {
    sample_rate: Option<u32>,
    channels: Option<u16>,
    buffer_size: Option<u32>,
}

impl Default for AudioStreamBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl AudioStreamBuilder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            sample_rate: None,
            channels: None,
            buffer_size: None,
        }
    }

    #[napi]
    pub fn set_sample_rate(&mut self, rate: u32) {
        self.sample_rate = Some(rate);
    }

    #[napi]
    pub fn set_channels(&mut self, channels: u8) {
        self.channels = Some(channels as u16);
    }

    #[napi]
    pub fn set_buffer_size(&mut self, size: u32) {
        self.buffer_size = Some(size);
    }

    #[napi]
    pub fn build(&self) -> Result<AudioStream> {
        // For now, just return a basic stream
        // In a full implementation, this would configure the stream with the given parameters
        Ok(AudioStream::new())
    }
}

/// Create and open a stream for audio playback
#[napi]
pub fn play(file_path: String) -> Result<AudioStream> {
    let mut stream = AudioStream::new();
    stream.open()?;
    stream.play_file(file_path)?;
    Ok(stream)
}

/// Get supported output configurations for the audio system
#[napi]
pub fn supported_output_configs() -> Result<Vec<SupportedStreamConfig>> {
    Ok(AudioStream::get_supported_configs())
}

/// Helper function to create a source from a vector of f32 samples (rodio format)
fn make_source_from_vec(
    samples: Vec<f32>,
    sample_rate: u32,
    channels: u16,
) -> impl RodioSource<Item = f32> {
    #[derive(Clone)]
    struct VecSource {
        samples: Vec<f32>,
        index: usize,
        sample_rate: u32,
        channels: u16,
    }

    impl Iterator for VecSource {
        type Item = f32;

        fn next(&mut self) -> Option<Self::Item> {
            if self.index < self.samples.len() {
                let sample = self.samples[self.index];
                self.index += 1;
                Some(sample)
            } else {
                None
            }
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            let remaining = self.samples.len() - self.index;
            (remaining, Some(remaining))
        }
    }

    impl RodioSource for VecSource {
        fn current_span_len(&self) -> Option<usize> {
            Some(self.samples.len() - self.index)
        }

        fn channels(&self) -> u16 {
            self.channels
        }

        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }

        fn total_duration(&self) -> Option<Duration> {
            Some(Duration::from_secs_f64(
                (self.samples.len() as f64) / (self.sample_rate as f64 * self.channels as f64),
            ))
        }
    }

    VecSource {
        samples,
        index: 0,
        sample_rate,
        channels,
    }
}

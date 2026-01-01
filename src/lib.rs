//! Audio FFI - High-performance native audio playback for Node.js/Bun
//! Implementation with rodio (pure Rust audio library)

#![allow(clippy::arc_with_non_send_sync)]

use base64::{engine::general_purpose, Engine as _};
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
// Usaremos OutputStream en cada instancia de AudioPlayer

/// Audio device information structure
#[napi(object)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Audio player state enumeration
#[napi]
#[derive(Debug, PartialEq)]
pub enum PlaybackState {
    Stopped = 0,
    Loaded = 1,
    Playing = 2,
    Paused = 3,
}

/// Thread-safe audio player with rodio backend
#[napi]
pub struct AudioPlayer {
    current_file: Option<String>,
    volume: Arc<Mutex<f32>>,
    state: Arc<Mutex<PlaybackState>>,
    duration: Arc<Mutex<f64>>,
    sink: Arc<Mutex<Option<Sink>>>,
    output_stream: Arc<Mutex<Option<OutputStream>>>,
    stream_handle: Arc<Mutex<Option<OutputStreamHandle>>>,
    audio_buffer: Arc<Mutex<Option<Vec<u8>>>>,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self {
            current_file: None,
            volume: Arc::new(Mutex::new(1.0)),
            state: Arc::new(Mutex::new(PlaybackState::Stopped)),
            duration: Arc::new(Mutex::new(0.0)),
            sink: Arc::new(Mutex::new(None)),
            output_stream: Arc::new(Mutex::new(None)),
            stream_handle: Arc::new(Mutex::new(None)),
            audio_buffer: Arc::new(Mutex::new(None)),
        }
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.stop().ok();
    }
}

#[napi]
impl AudioPlayer {
    #[napi(constructor)]
    pub fn new() -> Result<Self> {
        Ok(Self::default())
    }

    #[napi]
    pub fn get_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        // Rodio doesn't provide device enumeration in the same way as miniaudio
        // We'll return a default device
        Ok(vec![AudioDeviceInfo {
            id: "default".to_string(),
            name: "Default Output Device".to_string(),
            is_default: true,
        }])
    }

    #[napi]
    pub fn load_file(&mut self, file_path: String) -> Result<()> {
        let path = Path::new(&file_path);
        if !path.exists() {
            return Err(Error::new(
                Status::InvalidArg,
                format!("File not found: {}", file_path),
            ));
        }

        // Stop current playback
        self.stop().ok();

        // Estimate duration by opening the file and getting its properties
        let file = File::open(path)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to open file: {}", e)))?;
        let reader = BufReader::new(file);
        let _decoder = Decoder::new(reader).map_err(|e| {
            Error::new(
                Status::InvalidArg,
                format!("Failed to create decoder: {}", e),
            )
        })?;

        // Rodio doesn't provide direct duration info, so we'll estimate 0 for now
        *self.duration.lock().unwrap() = 0.0;
        self.current_file = Some(file_path);
        *self.state.lock().unwrap() = PlaybackState::Loaded;

        Ok(())
    }

    #[napi]
    pub fn load_buffer(&mut self, audio_data: Vec<u8>) -> Result<()> {
        if audio_data.is_empty() {
            return Err(Error::new(Status::InvalidArg, "Audio buffer is empty"));
        }

        // Stop current playback
        self.stop().ok();

        // Validate the audio data by trying to create a decoder
        let cursor = Cursor::new(audio_data.clone());
        let _decoder = Decoder::new(cursor).map_err(|e| {
            Error::new(
                Status::InvalidArg,
                format!("Failed to decode audio buffer: {}", e),
            )
        })?;

        // Store the audio data for playback
        *self.duration.lock().unwrap() = 0.0;
        *self.audio_buffer.lock().unwrap() = Some(audio_data);
        self.current_file = Some(format!(
            "__BUFFER__{}",
            std::time::SystemTime::now().elapsed().unwrap().as_millis()
        ));
        *self.state.lock().unwrap() = PlaybackState::Loaded;

        Ok(())
    }

    #[napi]
    pub fn load_base64(&mut self, base64_data: String) -> Result<()> {
        if base64_data.is_empty() {
            return Err(Error::new(Status::InvalidArg, "Base64 data is empty"));
        }

        // Decode base64 to bytes using the standard engine
        let audio_data = match general_purpose::STANDARD.decode(&base64_data) {
            Ok(data) => data,
            Err(e) => {
                return Err(Error::new(
                    Status::InvalidArg,
                    format!("Failed to decode base64: {}", e),
                ));
            }
        };

        self.load_buffer(audio_data)
    }

    #[napi]
    pub fn play(&mut self) -> Result<()> {
        let current_state = *self.state.lock().unwrap();
        if current_state != PlaybackState::Playing {
            // Check if we have a buffer or file to play
            let has_buffer = self.audio_buffer.lock().unwrap().is_some();
            let has_file = self.current_file.is_some();

            if !has_buffer && !has_file {
                return Err(Error::new(Status::InvalidArg, "Player not initialized"));
            }

            // Create output stream if not exists
            let mut stream_handle_guard = self.stream_handle.lock().unwrap();
            let mut output_stream_guard = self.output_stream.lock().unwrap();

            if stream_handle_guard.is_none() {
                match OutputStream::try_default() {
                    Ok((stream, handle)) => {
                        *stream_handle_guard = Some(handle);
                        *output_stream_guard = Some(stream);
                    }
                    Err(e) => {
                        return Err(Error::new(
                            Status::GenericFailure,
                            format!("Failed to create output stream: {}", e),
                        ));
                    }
                }
            }

            let stream_handle = stream_handle_guard.as_ref().unwrap();
            match Sink::try_new(stream_handle) {
                Ok(sink) => {
                    // Set volume BEFORE appending the source to prevent audio cut-off
                    // This ensures the correct volume is applied from the very first sample
                    let volume = *self.volume.lock().unwrap();
                    sink.set_volume(volume);

                    // Check if we have buffer data first
                    if let Some(buffer_data) = self.audio_buffer.lock().unwrap().clone() {
                        // Play from buffer
                        let cursor = Cursor::new(buffer_data);
                        let source = Decoder::new(cursor).map_err(|e| {
                            Error::new(
                                Status::InvalidArg,
                                format!("Failed to create decoder from buffer: {}", e),
                            )
                        })?;

                        sink.append(source);
                    } else if let Some(file_path) = &self.current_file {
                        // Play from file
                        let path = Path::new(file_path);
                        let file = File::open(path).map_err(|e| {
                            Error::new(Status::InvalidArg, format!("Failed to open file: {}", e))
                        })?;
                        let reader = BufReader::new(file);
                        let source = Decoder::new(reader).map_err(|e| {
                            Error::new(
                                Status::InvalidArg,
                                format!("Failed to create decoder: {}", e),
                            )
                        })?;

                        sink.append(source);
                    }

                    *self.sink.lock().unwrap() = Some(sink);
                    *self.state.lock().unwrap() = PlaybackState::Playing;
                }
                Err(e) => {
                    return Err(Error::new(
                        Status::GenericFailure,
                        format!("Failed to create sink: {}", e),
                    ));
                }
            }
        }

        Ok(())
    }

    #[napi]
    pub fn pause(&mut self) -> Result<()> {
        if self.current_file.is_none() {
            return Err(Error::new(Status::InvalidArg, "Player not initialized"));
        }

        let state = *self.state.lock().unwrap();
        if state == PlaybackState::Playing {
            if let Some(sink) = self.sink.lock().unwrap().as_ref() {
                sink.pause();
                *self.state.lock().unwrap() = PlaybackState::Paused;
            }
        }

        Ok(())
    }

    #[napi]
    pub fn stop(&mut self) -> Result<()> {
        if self.current_file.is_none() && self.audio_buffer.lock().unwrap().is_none() {
            return Err(Error::new(Status::InvalidArg, "Player not initialized"));
        }

        // Stop playback
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.stop();
        }

        // Clear sink and buffer
        *self.sink.lock().unwrap() = None;
        *self.audio_buffer.lock().unwrap() = None;

        *self.state.lock().unwrap() = PlaybackState::Stopped;
        *self.duration.lock().unwrap() = 0.0;
        self.current_file = None;

        Ok(())
    }

    #[napi]
    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(Error::new(
                Status::InvalidArg,
                "Volume must be between 0.0 and 1.0",
            ));
        }

        let vol = volume as f32;
        *self.volume.lock().unwrap() = vol;

        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.set_volume(vol);
        }

        Ok(())
    }

    #[napi]
    pub fn get_volume(&self) -> Result<f64> {
        Ok(*self.volume.lock().unwrap() as f64)
    }

    #[napi]
    pub fn is_playing(&self) -> bool {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            !sink.is_paused() && !sink.empty()
        } else {
            false
        }
    }

    #[napi]
    pub fn get_state(&self) -> PlaybackState {
        *self.state.lock().unwrap()
    }

    #[napi]
    pub fn get_duration(&self) -> Result<f64> {
        // Rodio doesn't provide duration info easily
        Ok(*self.duration.lock().unwrap())
    }

    #[napi]
    pub fn get_current_time(&self) -> Result<f64> {
        // Rodio doesn't provide position info easily
        Ok(0.0)
    }

    #[napi]
    pub fn get_current_file(&self) -> Option<String> {
        self.current_file.clone()
    }
}

#[napi]
pub fn initialize_audio() -> Result<String> {
    // Try to create an output stream to test audio system
    match OutputStream::try_default() {
        Ok((_stream, _handle)) => Ok("Audio system initialized with rodio".to_string()),
        Err(e) => Err(Error::new(
            Status::GenericFailure,
            format!("Failed to initialize audio: {}", e),
        )),
    }
}

#[napi]
pub fn get_supported_formats() -> Vec<String> {
    vec![
        "wav".to_string(),
        "mp3".to_string(),
        "flac".to_string(),
        "ogg".to_string(),
        "aac".to_string(),
        "m4a".to_string(),
        "opus".to_string(),
    ]
}

#[napi]
pub fn is_format_supported(format: String) -> bool {
    get_supported_formats().contains(&format.to_lowercase())
}

#[napi]
pub fn get_audio_info() -> Result<String> {
    Ok("Audio system: rodio\nDefault device: Default Output Device\nChannels: Stereo\nSample rate: 44100".to_string())
}

#[napi]
pub fn test_tone(frequency: f64, duration_ms: u32) -> Result<()> {
    use rodio::source::{SineWave, Source};

    let (_stream, handle) = OutputStream::try_default().map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to create output stream: {}", e),
        )
    })?;

    let sink = Sink::try_new(&handle).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to create sink: {}", e),
        )
    })?;

    let source = SineWave::new(frequency as f32)
        .take_duration(Duration::from_millis(duration_ms as u64))
        .amplify(0.3);

    sink.append(source);

    // Wait for tone to finish
    thread::sleep(Duration::from_millis(duration_ms as u64));

    Ok(())
}

#[napi(object)]
pub struct AudioPlayerConfig {
    pub volume: Option<f64>,
    pub auto_play: Option<bool>,
}

#[napi]
pub fn create_audio_player(config: Option<AudioPlayerConfig>) -> Result<AudioPlayer> {
    let mut player = AudioPlayer::new()?;

    if let Some(cfg) = config.as_ref() {
        if let Some(vol) = cfg.volume {
            player.set_volume(vol)?;
        }
    }

    Ok(player)
}

#[napi]
pub fn quick_play(file_path: String, config: Option<AudioPlayerConfig>) -> Result<AudioPlayer> {
    let mut player = AudioPlayer::new()?;

    if let Some(cfg) = config.as_ref() {
        if let Some(vol) = cfg.volume {
            player.set_volume(vol)?;
        }
    }

    player.load_file(file_path)?;

    let auto_play = config.as_ref().and_then(|c| c.auto_play).unwrap_or(false);

    if auto_play {
        player.play()?;
    }

    Ok(player)
}

#[napi(object)]
pub struct AudioMetadata {
    pub duration: f64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

#[napi]
pub fn get_audio_metadata(file_path: String) -> Result<AudioMetadata> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(Error::new(
            Status::InvalidArg,
            format!("File not found: {}", file_path),
        ));
    }

    // Rodio doesn't provide metadata extraction easily
    // We'll return basic info
    Ok(AudioMetadata {
        duration: 0.0,
        title: None,
        artist: None,
        album: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formats() {
        assert!(is_format_supported("mp3".to_string()));
        assert!(!is_format_supported("xyz".to_string()));
    }

    #[test]
    fn test_player_creation() {
        let player = AudioPlayer::new().unwrap();
        assert_eq!(player.get_volume().unwrap(), 1.0);
        assert_eq!(player.get_state(), PlaybackState::Stopped);
    }

    #[test]
    fn test_initialize_audio() {
        // Allow test to pass in CI environments without audio hardware
        // This is common in GitHub Actions runners on all platforms
        let result = initialize_audio();

        // Check if we're in a CI environment
        let is_ci = std::env::var("CI").is_ok()
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("CONTINUOUS_INTEGRATION").is_ok();

        if is_ci {
            // CI environments often don't have audio hardware available
            // Allow the test to pass if this is a known CI limitation
            if result.is_err() {
                println!(
                    "Warning: Audio initialization failed (expected in CI without audio hardware)"
                );
                return;
            }
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_devices() {
        let player = AudioPlayer::new().unwrap();
        let devices = player.get_devices().unwrap();
        assert!(!devices.is_empty());
    }
}

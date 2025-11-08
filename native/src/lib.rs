//! MiniAudio FFI - High-performance native audio playback for Node.js/Bun
//!
//! This module provides Rust-based audio playback capabilities using the rodio library.
//! It exposes a native addon interface that can be consumed from JavaScript/TypeScript.
//!
//! Features:
//! - Multi-format audio support (WAV, MP3, FLAC, OGG)
//! - Cross-platform compatibility (Windows, macOS, Linux)
//! - Memory-safe Rust implementation
//! - Async playback support
//! - Device enumeration
//! - Volume control
//! - Playback state management

#[macro_use]
extern crate napi_derive;

use napi::bindgen_prelude::*;
use napi::Result as NapiResult;
use std::sync::{Arc, Mutex};
use std::path::Path;
use rodio::{OutputStream, Sink, Decoder};
use std::fs::File;
use std::io::BufReader;


/// Audio device information structure
#[napi(object)]
pub struct AudioDeviceInfo {
    /// Unique device identifier
    pub id: String,
    /// Human-readable device name
    pub name: String,
    /// Whether this is the default output device
    pub is_default: bool,
}

/// Audio player state enumeration
#[napi]
pub enum PlaybackState {
    /// No audio loaded
    Stopped = 0,
    /// Audio loaded but not playing
    Loaded = 1,
    /// Currently playing audio
    Playing = 2,
    /// Playback paused
    Paused = 3,
}

/// Audio player implementation with thread-safe state management
#[napi]
pub struct AudioPlayer {
    /// Output stream (kept alive for the duration of the player)
    _stream: Option<OutputStream>,
    /// Audio sink for playback control (wrapped in Arc<Mutex> for thread safety)
    sink: Option<Arc<Mutex<Sink>>>,
    /// Currently loaded file path
    current_file: Option<String>,
    /// Current volume level (0.0 to 1.0)
    volume: f32,
    /// Playback state
    state: PlaybackState,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self {
            _stream: None,
            sink: None,
            current_file: None,
            volume: 1.0,
            state: PlaybackState::Stopped,
        }
    }
}

#[napi]
impl AudioPlayer {
    /// Create a new audio player instance
    ///
    /// # Returns
    ///
    /// A new `AudioPlayer` instance
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get available audio output devices
    ///
    /// # Returns
    ///
    /// Vector of `AudioDeviceInfo` structures representing available devices
    ///
    /// # Note
    ///
    /// rodio doesn't provide direct device enumeration, so this returns a default device
    /// Future versions may implement proper device enumeration
    #[napi]
    pub fn get_devices(&mut self) -> NapiResult<Vec<AudioDeviceInfo>> {
        let devices = vec![AudioDeviceInfo {
            id: "default".to_string(),
            name: "Default Output Device".to_string(),
            is_default: true,
        }];
        Ok(devices)
    }

    /// Load an audio file for playback
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the audio file to load
    ///
    /// # Returns
    ///
    /// Ok(()) if successful, Err if the file cannot be loaded
    ///
    /// # Errors
    ///
    /// - File does not exist
    /// - File format is not supported
    /// - Audio system initialization fails
    #[napi]
    pub fn load_file(&mut self, file_path: String) -> NapiResult<()> {
        let path = Path::new(&file_path);

        if !path.exists() {
            return Err(Error::new(
                Status::InvalidArg,
                format!("File not found: {}", file_path),
            ));
        }

        // Get output stream
        let (_stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to create output stream: {}", e)))?;

        // Open the audio file
        let file = File::open(path)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to open file: {}", e)))?;

        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| Error::new(Status::InvalidArg, format!("Unsupported audio format: {}", e)))?;

        // Create sink and load the source
        let sink = Sink::try_new(&stream_handle)
            .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to create sink: {}", e)))?;

        sink.append(source);
        sink.pause(); // Start in paused state

        // Store the player state
        self._stream = Some(_stream);
        self.sink = Some(Arc::new(Mutex::new(sink)));
        self.current_file = Some(file_path);
        self.state = PlaybackState::Loaded;

        Ok(())
    }

    /// Start or resume audio playback
    ///
    /// # Returns
    ///
    /// Ok(()) if successful, Err if no file is loaded
    ///
    /// # Errors
    ///
    /// - No audio file has been loaded
    /// - Audio playback fails to start
    #[napi]
    pub fn play(&mut self) -> NapiResult<()> {
        if let Some(sink) = &self.sink {
            let sink = sink.lock().unwrap();
            if !sink.empty() {
                sink.play();
                self.state = PlaybackState::Playing;
                Ok(())
            } else {
                Err(Error::new(Status::InvalidArg, "No audio loaded".to_string()))
            }
        } else {
            Err(Error::new(Status::InvalidArg, "Player not initialized".to_string()))
        }
    }

    /// Pause the current audio playback
    ///
    /// # Returns
    ///
    /// Ok(()) if successful, Err if no player is initialized
    #[napi]
    pub fn pause(&mut self) -> NapiResult<()> {
        if let Some(sink) = &self.sink {
            let sink = sink.lock().unwrap();
            sink.pause();
            self.state = PlaybackState::Paused;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "Player not initialized".to_string()))
        }
    }

    /// Stop audio playback and clear the audio queue
    ///
    /// # Returns
    ///
    /// Ok(()) if successful, Err if no player is initialized
    #[napi]
    pub fn stop(&mut self) -> NapiResult<()> {
        if let Some(sink) = &self.sink {
            let sink = sink.lock().unwrap();
            sink.stop();
            self.state = PlaybackState::Stopped;
            Ok(())
        } else {
            Err(Error::new(Status::InvalidArg, "Player not initialized".to_string()))
        }
    }

    /// Set the audio volume level
    ///
    /// # Arguments
    ///
    /// * `volume` - Volume level between 0.0 (silent) and 1.0 (maximum)
    ///
    /// # Returns
    ///
    /// Ok(()) if successful, Err if volume is out of range
    ///
    /// # Errors
    ///
    /// - Volume is not between 0.0 and 1.0
    #[napi]
    pub fn set_volume(&mut self, volume: f64) -> NapiResult<()> {
        if volume < 0.0 || volume > 1.0 {
            return Err(Error::new(
                Status::InvalidArg,
                "Volume must be between 0.0 and 1.0".to_string(),
            ));
        }

        self.volume = volume as f32;

        if let Some(sink) = &self.sink {
            let sink = sink.lock().unwrap();
            sink.set_volume(self.volume);
        }

        Ok(())
    }

    /// Get the current volume level
    ///
    /// # Returns
    ///
    /// Current volume as a value between 0.0 and 1.0
    #[napi]
    pub fn get_volume(&self) -> NapiResult<f64> {
        Ok(self.volume as f64)
    }

    /// Check if audio is currently playing
    ///
    /// # Returns
    ///
    /// true if audio is playing, false otherwise
    #[napi]
    pub fn is_playing(&self) -> bool {
        if let Some(sink) = &self.sink {
            let sink = sink.lock().unwrap();
            !sink.is_paused() && !sink.empty()
        } else {
            false
        }
    }

    /// Get the current playback state
    ///
    /// # Returns
    ///
    /// Current `PlaybackState` of the player
    #[napi]
    pub fn get_state(&self) -> PlaybackState {
        self.state.clone()
    }

    /// Get the total duration of the loaded audio file
    ///
    /// # Returns
    ///
    /// Duration in seconds, or 0.0 if not available
    ///
    /// # Note
    ///
    /// rodio doesn't provide direct duration information.
    /// This will be implemented in a future version using metadata extraction.
    #[napi]
    pub fn get_duration(&self) -> NapiResult<f64> {
        // TODO: Implement duration calculation using metadata extraction
        Ok(0.0)
    }

    /// Get the current playback position
    ///
    /// # Returns
    ///
    /// Current position in seconds, or 0.0 if not available
    ///
    /// # Note
    ///
    /// rodio doesn't provide direct position information.
    /// This requires implementing custom position tracking.
    #[napi]
    pub fn get_current_time(&self) -> NapiResult<f64> {
        // TODO: Implement position tracking
        Ok(0.0)
    }

    /// Get the path of the currently loaded audio file
    ///
    /// # Returns
    ///
    /// Current file path as Option<String>, or None if no file is loaded
    #[napi]
    pub fn get_current_file(&self) -> Option<String> {
        self.current_file.clone()
    }
}

/// Get the list of supported audio formats
///
/// # Returns
///
/// Vector of strings representing supported audio format extensions
#[napi]
pub fn get_supported_formats() -> Vec<String> {
    vec![
        "wav".to_string(),
        "mp3".to_string(),
        "flac".to_string(),
        "ogg".to_string(),
        "m4a".to_string(),
        "aac".to_string(),
    ]
}

/// Initialize the audio system
///
/// # Returns
///
/// Success message if audio system initializes correctly
///
/// # Errors
///
/// Returns error if audio system cannot be initialized
#[napi]
pub fn initialize_audio() -> NapiResult<String> {
    // Test if we can create an output stream
    match OutputStream::try_default() {
        Ok(_) => Ok("Audio system initialized successfully".to_string()),
        Err(e) => Err(Error::new(
            Status::GenericFailure,
            format!("Failed to initialize audio system: {}", e),
        )),
    }
}

/// Check if a specific audio format is supported
///
/// # Arguments
///
/// * `format` - Audio format to check (e.g., "mp3", "wav")
///
/// # Returns
///
/// true if format is supported, false otherwise
#[napi]
pub fn is_format_supported(format: String) -> bool {
    let supported_formats = get_supported_formats();
    supported_formats.contains(&format.to_lowercase())
}

/// Get audio system information
///
/// # Returns
///
/// Object containing audio system information
#[napi]
pub fn get_audio_info() -> NapiResult<String> {
    Ok("Audio system info: rodio backend with cross-platform support".to_string())
}

/// Test audio playback with a simple tone generator
///
/// # Arguments
///
/// * `frequency` - Frequency of the test tone in Hz
/// * `duration_ms` - Duration of the test tone in milliseconds
///
/// # Returns
///
/// Ok(()) if successful, Err if playback fails
///
/// # Note
///
/// This is a utility function for testing the audio system
#[napi]
pub fn test_tone(frequency: f64, duration_ms: u32) -> NapiResult<()> {
    // TODO: Implement tone generator using rodio's Source trait
    // For now, just validate the parameters
    if frequency <= 0.0 || frequency > 20000.0 {
        return Err(Error::new(
            Status::InvalidArg,
            "Frequency must be between 0 and 20000 Hz".to_string(),
        ));
    }

    if duration_ms == 0 || duration_ms > 60000 {
        return Err(Error::new(
            Status::InvalidArg,
            "Duration must be between 1 and 60000 ms".to_string(),
        ));
    }

    Ok(())
}

/// Create a new audio player with optional configuration
///
/// # Arguments
///
/// * `config` - Optional player configuration
///
/// # Returns
///
/// A new `AudioPlayer` instance
#[napi]
pub fn create_audio_player(config: Option<AudioPlayerConfig>) -> AudioPlayer {
    let mut player = AudioPlayer::new();

    if let Some(config) = config {
        if let Some(volume) = config.volume {
            let _ = player.set_volume(volume);
        }
    }

    player
}

/// Audio player configuration
#[napi(object)]
#[derive(Clone)]
pub struct AudioPlayerConfig {
    /// Volume level (0.0 to 1.0)
    pub volume: Option<f64>,
    /// Whether to loop playback
    pub loop_playback: Option<bool>,
    /// Auto-play when file is loaded
    pub auto_play: Option<bool>,
}

/// Quick play utility for simple audio playback
///
/// # Arguments
///
/// * `file_path` - Path to the audio file
/// * `config` - Optional playback configuration
///
/// # Returns
///
/// An `AudioPlayer` instance with the loaded file
#[napi]
pub fn quick_play(file_path: String, config: Option<AudioPlayerConfig>) -> NapiResult<AudioPlayer> {
    let auto_play = config.as_ref()
        .map(|c| c.auto_play.unwrap_or(false))
        .unwrap_or(false);

    let mut player = create_audio_player(config);
    player.load_file(file_path)?;

    if auto_play {
        player.play()?;
    }

    Ok(player)
}

/// Get audio metadata from file
///
/// # Arguments
///
/// * `file_path` - Path to the audio file
///
/// # Returns
///
/// Audio metadata as JSON string
///
/// # Note
///
/// This is a placeholder implementation. In a real implementation,
/// you would use a metadata extraction library.
#[napi]
pub fn get_audio_metadata(file_path: String) -> NapiResult<String> {
    let path = Path::new(&file_path);

    if !path.exists() {
        return Err(Error::new(
            Status::InvalidArg,
            format!("File not found: {}", file_path),
        ));
    }

    // TODO: Implement real metadata extraction using a library like symphonia
    let metadata = format!(
        r#"{{
  "file_path": "{}",
  "file_size": "unknown",
  "duration": "unknown",
  "sample_rate": "unknown",
  "channels": "unknown",
  "bitrate": "unknown",
  "format": "unknown"
}}"#,
        file_path
    );

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_formats() {
        let formats = get_supported_formats();
        assert!(formats.contains(&"mp3".to_string()));
        assert!(formats.contains(&"wav".to_string()));
    }

    #[test]
    fn test_format_support() {
        assert!(is_format_supported("mp3".to_string()));
        assert!(!is_format_supported("unknown".to_string()));
    }

    #[test]
    fn test_audio_player_creation() {
        let player = AudioPlayer::new();
        assert_eq!(player.get_volume().unwrap(), 1.0);
        assert_eq!(player.get_state(), PlaybackState::Stopped);
    }
}

//! MiniAudio FFI - High-performance native audio playback for Node.js/Bun

use napi::{Result, Error, Status};
use napi_derive::napi;
use std::sync::{Arc, Mutex};
use std::path::Path;
use rodio::{OutputStream, Sink, Decoder};
use std::fs::File;
use std::io::BufReader;

/// Audio device information structure
#[napi(object)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Audio player state enumeration
#[napi]
pub enum PlaybackState {
    Stopped = 0,
    Loaded = 1,
    Playing = 2,
    Paused = 3,
}

/// Thread-safe audio player with proper resource management
#[napi]
pub struct AudioPlayer {
    _stream: Option<Arc<OutputStream>>,
    sink: Option<Arc<Mutex<Sink>>>,
    current_file: Option<String>,
    volume: f32,
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
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[napi]
    pub fn get_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
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
            return Err(Error::new(Status::InvalidArg, format!("File not found: {}", file_path)));
        }

        self.stop().ok();

        let (stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| Error::new(Status::GenericFailure, format!("Stream error: {}", e)))?;

        let file = File::open(path)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Open error: {}", e)))?;

        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| Error::new(Status::InvalidArg, format!("Decode error: {}", e)))?;

        let sink = Sink::try_new(&stream_handle)
            .map_err(|e| Error::new(Status::GenericFailure, format!("Sink error: {}", e)))?;

        sink.append(source);
        sink.pause();

        self._stream = Some(Arc::new(stream));
        self.sink = Some(Arc::new(Mutex::new(sink)));
        self.current_file = Some(file_path);
        self.state = PlaybackState::Loaded;

        Ok(())
    }

    #[napi]
    pub fn play(&mut self) -> Result<()> {
        let sink = self.sink.as_ref()
            .ok_or_else(|| Error::new(Status::InvalidArg, "Player not initialized"))?;
        
        let guard = sink.lock()
            .map_err(|e| Error::new(Status::GenericFailure, format!("Lock error: {}", e)))?;
        
        if guard.empty() {
            return Err(Error::new(Status::InvalidArg, "No audio loaded"));
        }

        guard.play();
        drop(guard);
        self.state = PlaybackState::Playing;
        Ok(())
    }

    #[napi]
    pub fn pause(&mut self) -> Result<()> {
        let sink = self.sink.as_ref()
            .ok_or_else(|| Error::new(Status::InvalidArg, "Player not initialized"))?;
        
        let guard = sink.lock()
            .map_err(|e| Error::new(Status::GenericFailure, format!("Lock error: {}", e)))?;
        
        guard.pause();
        drop(guard);
        self.state = PlaybackState::Paused;
        Ok(())
    }

    #[napi]
    pub fn stop(&mut self) -> Result<()> {
        let sink = self.sink.as_ref()
            .ok_or_else(|| Error::new(Status::InvalidArg, "Player not initialized"))?;
        
        let guard = sink.lock()
            .map_err(|e| Error::new(Status::GenericFailure, format!("Lock error: {}", e)))?;
        
        guard.stop();
        drop(guard);
        self.state = PlaybackState::Stopped;
        Ok(())
    }

    #[napi]
    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(Error::new(Status::InvalidArg, "Volume must be between 0.0 and 1.0"));
        }

        self.volume = volume as f32;
        if let Some(sink) = &self.sink {
            if let Ok(guard) = sink.lock() {
                guard.set_volume(self.volume);
            }
        }
        Ok(())
    }

    #[napi]
    pub fn get_volume(&self) -> Result<f64> {
        Ok(self.volume as f64)
    }

    #[napi]
    pub fn is_playing(&self) -> bool {
        self.sink.as_ref()
            .and_then(|s| s.lock().ok())
            .map(|g| !g.is_paused() && !g.empty())
            .unwrap_or(false)
    }

    #[napi]
    pub fn get_state(&self) -> PlaybackState {
        self.state
    }

    #[napi]
    pub fn get_duration(&self) -> Result<f64> {
        Ok(0.0)
    }

    #[napi]
    pub fn get_current_time(&self) -> Result<f64> {
        Ok(0.0)
    }

    #[napi]
    pub fn get_current_file(&self) -> Option<String> {
        self.current_file.clone()
    }
}

#[napi]
pub fn initialize_audio() -> Result<String> {
    OutputStream::try_default()
        .map(|_| "Audio system initialized".to_string())
        .map_err(|e| Error::new(Status::GenericFailure, format!("Init error: {}", e)))
}

#[napi]
pub fn get_supported_formats() -> Vec<String> {
    vec!["wav".to_string(), "mp3".to_string(), "flac".to_string(), "ogg".to_string()]
}

#[napi]
pub fn is_format_supported(format: String) -> bool {
    get_supported_formats().contains(&format.to_lowercase())
}

#[napi]
pub fn get_audio_info() -> Result<String> {
    Ok("Audio system: rodio + symphonia".to_string())
}

#[napi]
pub fn test_tone(_frequency: f64, _duration_ms: u32) -> Result<()> {
    Err(Error::new(Status::GenericFailure, "Not implemented yet"))
}

#[napi(object)]
pub struct AudioPlayerConfig {
    pub volume: Option<f64>,
    pub auto_play: Option<bool>,
}

#[napi]
pub fn create_audio_player(config: Option<AudioPlayerConfig>) -> Result<AudioPlayer> {
    let mut player = AudioPlayer::new();
    
    if let Some(cfg) = config.as_ref() {
        if let Some(vol) = cfg.volume {
            player.set_volume(vol)?;
        }
    }
    
    Ok(player)
}

#[napi]
pub fn quick_play(file_path: String, config: Option<AudioPlayerConfig>) -> Result<AudioPlayer> {
    let mut player = AudioPlayer::new();
    
    if let Some(cfg) = config.as_ref() {
        if let Some(vol) = cfg.volume {
            player.set_volume(vol)?;
        }
    }
    
    player.load_file(file_path)?;
    
    let auto_play = config.as_ref()
        .and_then(|c| c.auto_play)
        .unwrap_or(false);
    
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
        return Err(Error::new(Status::InvalidArg, format!("File not found: {}", file_path)));
    }

    // For now, return placeholder metadata
    // In a real implementation, you'd extract actual metadata from the audio file
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
    fn test_player() {
        let player = AudioPlayer::new();
        assert_eq!(player.get_volume().unwrap(), 1.0);
        assert_eq!(player.get_state(), PlaybackState::Stopped);
    }
}

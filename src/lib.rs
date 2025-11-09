//! Audio FFI - High-performance native audio playback for Node.js/Bun
//! Implementación con miniaudio-rs (decodificación + playback integrados)

use napi::{Error, Result, Status};
use napi_derive::napi;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// === Miniaudio Integration ===
use miniaudio::{
    Context, Device, DeviceConfig, DeviceType, Engine, Sound, SoundFlags, SoundConfig, Decoder,
    DecoderConfig, DeviceInfo, DeviceId, ma_result_callback,
};

/// Audio device information structure
#[napi(object)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Audio player state enumeration
#[napi]
#[derive(Debug, PartialEq, Clone)]
pub enum PlaybackState {
    Stopped = 0,
    Loaded = 1,
    Playing = 2,
    Paused = 3,
}

/// Thread-safe audio player with miniaudio backend
#[napi]
pub struct AudioPlayer {
    current_file: Option<String>,
    volume: Arc<Mutex<f32>>,
    state: Arc<Mutex<PlaybackState>>,
    duration: Arc<Mutex<f64>>,
    engine: Arc<Mutex<Option<Arc<Engine>>>>,
    sound: Arc<Mutex<Option<Sound>>>,
    playback_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self {
            current_file: None,
            volume: Arc::new(Mutex::new(1.0)),
            state: Arc::new(Mutex::new(PlaybackState::Stopped)),
            duration: Arc::new(Mutex::new(0.0)),
            engine: Arc::new(Mutex::new(None)),
            sound: Arc::new(Mutex::new(None)),
            playback_handle: Arc::new(Mutex::new(None)),
        }
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.stop().ok();
        if let Some(handle) = self.playback_handle.lock().unwrap().take() {
            handle.join().ok();
        }
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
        let context = Context::new(&[]).map_err(|e| {
            Error::new(Status::GenericFailure, format!("Context error: {}", e))
        })?;

        let playback_infos = context.get_playback_devices().map_err(|e| {
            Error::new(Status::GenericFailure, format!("Device enumeration error: {}", e))
        })?;

        let mut devices = Vec::new();
        let default_device_id = context.get_default_playback_device().map_err(|e| {
            Error::new(Status::GenericFailure, format!("Default device error: {}", e))
        })?;

        for (index, info) in playback_infos.iter().enumerate() {
            devices.push(AudioDeviceInfo {
                id: index.to_string(),
                name: info.name().to_string(),
                is_default: info.id() == default_device_id.as_ref(),
            });
        }

        Ok(devices)
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

        // Create engine if not exists
        let mut engine_guard = self.engine.lock().unwrap();
        if engine_guard.is_none() {
            let engine = Engine::new().map_err(|e| {
                Error::new(Status::GenericFailure, format!("Engine creation error: {}", e))
            })?;
            *engine_guard = Some(Arc::new(engine));
        }

        let engine = engine_guard.as_ref().unwrap().clone();

        // Create decoder to get duration
        let mut decoder = Decoder::from_file(path, DecoderConfig::default()).map_err(|e| {
            Error::new(Status::InvalidArg, format!("Decoder error: {}", e))
        })?;

        let duration = decoder.get_length_in_pcm_frames() as f64 / decoder.output_sample_rate() as f64;
        *self.duration.lock().unwrap() = duration;

        // Create sound from file
        let sound_config = SoundConfig::new();
        let sound = engine.create_sound_from_file(path, SoundFlags::default(), sound_config)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Sound creation error: {}", e)))?;

        // Set initial volume
        sound.set_volume(*self.volume.lock().unwrap());

        self.sound.lock().unwrap().replace(sound);
        self.current_file = Some(file_path);
        *self.state.lock().unwrap() = PlaybackState::Loaded;

        Ok(())
    }

    #[napi]
    pub fn play(&mut self) -> Result<()> {
        if self.sound.lock().unwrap().is_none() {
            return Err(Error::new(Status::InvalidArg, "No audio file loaded"));
        }

        let current_state = *self.state.lock().unwrap();
        if current_state != PlaybackState::Playing {
            // Start playback thread
            self.start_playback_thread()?;
            *self.state.lock().unwrap() = PlaybackState::Playing;
        }

        Ok(())
    }

    #[napi]
    pub fn pause(&mut self) -> Result<()> {
        let state = *self.state.lock().unwrap();
        if state == PlaybackState::Playing {
            if let Some(sound) = self.sound.lock().unwrap().as_ref() {
                sound.pause().map_err(|e| {
                    Error::new(Status::GenericFailure, format!("Pause error: {}", e))
                })?;
                *self.state.lock().unwrap() = PlaybackState::Paused;
            }
        }
        Ok(())
    }

    #[napi]
    pub fn stop(&mut self) -> Result<()> {
        // Stop playback
        if let Some(sound) = self.sound.lock().unwrap().as_ref() {
            sound.stop().map_err(|e| {
                Error::new(Status::GenericFailure, format!("Stop error: {}", e))
            })?;
        }

        // Clear sound
        *self.sound.lock().unwrap() = None;

        // Wait for thread
        if let Some(handle) = self.playback_handle.lock().unwrap().take() {
            handle.join().ok();
        }

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

        if let Some(sound) = self.sound.lock().unwrap().as_ref() {
            sound.set_volume(vol).map_err(|e| {
                Error::new(Status::GenericFailure, format!("Volume error: {}", e))
            })?;
        }

        Ok(())
    }

    #[napi]
    pub fn get_volume(&self) -> Result<f64> {
        Ok(*self.volume.lock().unwrap() as f64)
    }

    #[napi]
    pub fn is_playing(&self) -> bool {
        *self.state.lock().unwrap() == PlaybackState::Playing
    }

    #[napi]
    pub fn get_state(&self) -> PlaybackState {
        *self.state.lock().unwrap()
    }

    #[napi]
    pub fn get_duration(&self) -> Result<f64> {
        Ok(*self.duration.lock().unwrap())
    }

    #[napi]
    pub fn get_current_time(&self) -> Result<f64> {
        if let Some(sound) = self.sound.lock().unwrap().as_ref() {
            let time = sound.get_time_in_pcm_frames() as f64 /
                      sound.get_output_sample_rate() as f64;
            Ok(time)
        } else {
            Ok(0.0)
        }
    }

    #[napi]
    pub fn get_current_file(&self) -> Option<String> {
        self.current_file.clone()
    }
}

impl AudioPlayer {
    fn start_playback_thread(&mut self) -> Result<()> {
        let sound = self.sound.lock().unwrap().as_ref().unwrap().clone();
        let state = self.state.clone();
        let duration = self.duration.clone();

        // Start playing
        sound.start().map_err(|e| {
            Error::new(Status::GenericFailure, format!("Play error: {}", e))
        })?;

        // Create monitoring thread
        let handle = thread::spawn(move || {
            while *state.lock().unwrap() == PlaybackState::Playing {
                if sound.is_playing() {
                    thread::sleep(Duration::from_millis(50));

                    // Check if playback finished
                    if sound.at_end() {
                        *state.lock().unwrap() = PlaybackState::Stopped;
                        break;
                    }
                } else {
                    break;
                }
            }
        });

        *self.playback_handle.lock().unwrap() = Some(handle);
        Ok(())
    }
}

#[napi]
pub fn initialize_audio() -> Result<String> {
    let context = Context::new(&[]).map_err(|e| {
        Error::new(Status::GenericFailure, format!("Initialization error: {}", e))
    })?;

    if context.get_playback_devices().map_err(|e| {
        Error::new(Status::GenericFailure, format!("Device error: {}", e))
    })?.is_empty() {
        return Err(Error::new(Status::GenericFailure, "No audio devices found"));
    }

    Ok("Audio system initialized with miniaudio".to_string())
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
        "wma".to_string(),
    ]
}

#[napi]
pub fn is_format_supported(format: String) -> bool {
    get_supported_formats().contains(&format.to_lowercase())
}

#[napi]
pub fn get_audio_info() -> Result<String> {
    let context = Context::new(&[]).map_err(|e| {
        Error::new(Status::GenericFailure, format!("Context error: {}", e))
    })?;

    let default_device = context.get_default_playback_device().map_err(|e| {
        Error::new(Status::GenericFailure, format!("Device error: {}", e))
    })?;

    let info = context.get_device_info(&default_device).map_err(|e| {
        Error::new(Status::GenericFailure, format!("Info error: {}", e))
    })?;

    Ok(format!(
        "Audio system: miniaudio-rs\nDefault device: {}\nChannels: {}\nSample rate: {}",
        info.name(),
        info.max_channels(),
        info.min_sample_rate() // Approximate
    ))
}

#[napi]
pub fn test_tone(frequency: f64, duration_ms: u32) -> Result<()> {
    let engine = Engine::new().map_err(|e| {
        Error::new(Status::GenericFailure, format!("Engine error: {}", e))
    })?;

    engine.create_and_play_tone(
        frequency as f32,
        0.3, // volume
        duration_ms as f32 / 1000.0,
    ).map_err(|e| {
        Error::new(Status::GenericFailure, format!("Tone error: {}", e))
    })?;

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

    let mut decoder = Decoder::from_file(path, DecoderConfig::default()).map_err(|e| {
        Error::new(Status::InvalidArg, format!("Decoder error: {}", e))
    })?;

    let sample_rate = decoder.output_sample_rate() as f64;
    let duration = decoder.get_length_in_pcm_frames() as f64 / sample_rate;

    // Extract metadata from decoder (miniaudio provides this)
    let mut title = None;
    let mut artist = None;
    let mut album = None;

    // Miniaudio's decoder can provide metadata tags
    // This is a simplified version - actual implementation would parse tags
    if let Ok(tags) = decoder.get_metadata() {
        for tag in tags.iter() {
            match tag.key().to_lowercase().as_str() {
                "title" => title = Some(tag.value().to_string()),
                "artist" => artist = Some(tag.value().to_string()),
                "album" => album = Some(tag.value().to_string()),
                _ => {}
            }
        }
    }

    Ok(AudioMetadata {
        duration,
        title,
        artist,
        album,
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
        assert!(initialize_audio().is_ok());
    }

    #[test]
    fn test_devices() {
        let player = AudioPlayer::new().unwrap();
        let devices = player.get_devices().unwrap();
        assert!(!devices.is_empty());
    }
}

use base64::{engine::general_purpose, Engine as _};
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;
use std::sync::{Arc, Mutex};

// Importamos los tipos definidos en el otro módulo
use crate::debug_log;
use crate::types::{AudioDeviceInfo, AudioPlayerConfig, PlaybackState};

/// Thread-safe audio player with rodio backend
#[napi]
pub struct AudioPlayer {
    current_file: Option<String>,
    volume: Arc<Mutex<f32>>,
    state: Arc<Mutex<PlaybackState>>,
    duration: Arc<Mutex<f64>>,
    sink: Arc<Mutex<Option<Sink>>>,
    // OutputStream needs to be kept alive
    #[allow(dead_code)]
    output_stream: Arc<Mutex<Option<OutputStream>>>,
    audio_buffer: Arc<Mutex<Option<Vec<u8>>>>,
    // Track if player was ever initialized
    initialized: bool,
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
            audio_buffer: Arc::new(Mutex::new(None)),
            initialized: false,
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
        Ok(vec![AudioDeviceInfo {
            id: "default".to_string(),
            name: "Default Output Device".to_string(),
            is_default: true,
        }])
    }

    #[napi]
    pub fn load_file(&mut self, file_path: String) -> Result<()> {
        debug_log!("Loading file: {}", file_path);
        let path = Path::new(&file_path);
        if !path.exists() {
            debug_log!("File not found: {}", file_path);
            return Err(Error::new(
                Status::InvalidArg,
                format!("File not found: {}", file_path),
            ));
        }

        // Mark as initialized before stopping (to allow cleanup if player was used before)
        self.initialized = true;
        self.stop().ok();

        // Validate file opening
        let file = File::open(path)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to open file: {}", e)))?;
        let reader = BufReader::new(file);
        let decoder = Decoder::new(reader).map_err(|e| {
            Error::new(
                Status::InvalidArg,
                format!("Failed to create decoder: {}", e),
            )
        })?;

        // Calculate duration from decoder
        let duration = decoder
            .total_duration()
            .unwrap_or(std::time::Duration::ZERO);
        let duration_seconds =
            duration.as_secs() as f64 + duration.subsec_nanos() as f64 / 1_000_000_000.0;
        *self.duration.lock().unwrap() = duration_seconds;

        debug_log!(
            "File loaded successfully, duration: {} seconds",
            duration_seconds
        );
        self.current_file = Some(file_path);
        *self.state.lock().unwrap() = PlaybackState::Loaded;

        Ok(())
    }

    #[napi]
    pub fn load_buffer(&mut self, audio_data: Vec<u8>) -> Result<()> {
        debug_log!("Loading buffer ({} bytes)", audio_data.len());
        if audio_data.is_empty() {
            debug_log!("Audio buffer is empty");
            return Err(Error::new(Status::InvalidArg, "Audio buffer is empty"));
        }
        // Mark as initialized before stopping (to allow cleanup if player was used before)
        self.initialized = true;
        self.stop().ok();

        let cursor = Cursor::new(audio_data.clone());
        let _decoder = Decoder::new(cursor).map_err(|e| {
            Error::new(
                Status::InvalidArg,
                format!("Failed to decode buffer: {}", e),
            )
        })?;

        *self.duration.lock().unwrap() = 0.0;
        *self.audio_buffer.lock().unwrap() = Some(audio_data);
        self.current_file = Some(format!(
            "__BUFFER__{}",
            std::time::SystemTime::now().elapsed().unwrap().as_millis()
        ));
        *self.state.lock().unwrap() = PlaybackState::Loaded;
        debug_log!("Buffer loaded successfully");

        Ok(())
    }

    #[napi]
    pub fn load_base64(&mut self, base64_data: String) -> Result<()> {
        debug_log!("Loading base64 audio data");
        if base64_data.is_empty() {
            debug_log!("Base64 data is empty");
            return Err(Error::new(Status::InvalidArg, "Base64 data is empty"));
        }
        let audio_data = general_purpose::STANDARD
            .decode(&base64_data)
            .map_err(|e| {
                Error::new(
                    Status::InvalidArg,
                    format!("Failed to decode base64: {}", e),
                )
            })?;
        self.load_buffer(audio_data)
    }

    #[napi]
    pub fn play(&mut self) -> Result<()> {
        let has_buffer = self.audio_buffer.lock().unwrap().is_some();
        let has_file = self.current_file.is_some();

        if !has_buffer && !has_file {
            debug_log!("Play called but player not initialized");
            return Err(Error::new(Status::InvalidArg, "Player not initialized"));
        }

        let current_state = self.state.lock().unwrap().clone();
        debug_log!("Play called, current state: {:?}", current_state);
        if current_state != PlaybackState::Playing {
            let mut output_stream_guard = self.output_stream.lock().unwrap();
            if output_stream_guard.is_none() {
                debug_log!("Creating output stream...");
                let stream = OutputStreamBuilder::open_default_stream().map_err(|e| {
                    debug_log!("Failed to create output stream: {}", e);
                    Error::new(
                        Status::GenericFailure,
                        format!("Failed to create output stream: {}", e),
                    )
                })?;

                // We keep the stream alive
                *output_stream_guard = Some(stream);
                debug_log!("Output stream created");

                // Create sink using the mixer from the output stream
                let output_stream_ref = output_stream_guard.as_ref().unwrap();
                let sink = Sink::connect_new(output_stream_ref.mixer());

                *self.sink.lock().unwrap() = Some(sink);
            }

            // At this point sink should exist or be recreatable, logic slightly simplified for brevity
            // Note: Rodio's architecture usually requires keeping the Stream alive.
            // In the original code, `OutputStreamBuilder` was used which is slightly different.
            // Assuming we use the existing sink logic:

            let sink_guard = self.sink.lock().unwrap();
            if let Some(sink) = sink_guard.as_ref() {
                let volume = *self.volume.lock().unwrap();
                sink.set_volume(volume);
                debug_log!("Setting volume to: {}", volume);

                if sink.empty() {
                    debug_log!("Sink is empty, appending source...");
                    if let Some(buffer_data) = self.audio_buffer.lock().unwrap().clone() {
                        debug_log!("Playing from buffer ({} bytes)", buffer_data.len());
                        let cursor = Cursor::new(buffer_data);
                        let source = Decoder::new(cursor).unwrap();
                        sink.append(source);
                    } else if let Some(file_path) = &self.current_file {
                        debug_log!("Playing from file: {}", file_path);
                        let file = File::open(file_path).unwrap();
                        let source = Decoder::new(BufReader::new(file)).unwrap();
                        sink.append(source);
                    }
                    sink.play();
                    debug_log!("Sink playing");
                } else {
                    debug_log!("Resuming paused audio");
                    sink.play(); // Resume if paused
                }
            }

            *self.state.lock().unwrap() = PlaybackState::Playing;
            debug_log!("State set to Playing");
        }
        Ok(())
    }

    #[napi]
    pub fn pause(&mut self) -> Result<()> {
        debug_log!("Pause called");
        let current_state = self.state.lock().unwrap().clone();

        // If already paused or stopped with no sink, just update state
        if current_state == PlaybackState::Paused {
            debug_log!("Already paused, no action needed");
            return Ok(());
        }

        if current_state == PlaybackState::Stopped {
            debug_log!("Player is stopped, nothing to pause");
            return Ok(());
        }

        let sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_ref() {
            sink.pause();
            *self.state.lock().unwrap() = PlaybackState::Paused;
            debug_log!("State set to Paused");
            Ok(())
        } else {
            // Sink doesn't exist but player is in Playing/Loaded state
            // This can happen after stop() was called but before play()
            // Just update state to Paused since there's nothing playing
            *self.state.lock().unwrap() = PlaybackState::Paused;
            debug_log!("No sink available, state set to Paused anyway");
            Ok(())
        }
    }

    #[napi]
    pub fn stop(&mut self) -> Result<()> {
        debug_log!("Stop called");

        // Only error if player was never initialized
        if !self.initialized {
            debug_log!("Cannot stop - player not initialized");
            return Err(Error::new(Status::InvalidArg, "Player not initialized"));
        }

        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            debug_log!("Stopping sink");
            sink.stop();
        }
        *self.sink.lock().unwrap() = None;
        *self.audio_buffer.lock().unwrap() = None;
        *self.state.lock().unwrap() = PlaybackState::Stopped;
        self.current_file = None;
        debug_log!("State set to Stopped");
        Ok(())
    }

    #[napi]
    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        debug_log!("Setting volume to: {}", volume);
        if !(0.0..=1.0).contains(&volume) {
            debug_log!("Invalid volume range: {}", volume);
            return Err(Error::new(
                Status::InvalidArg,
                "Volume must be between 0.0 and 1.0",
            ));
        }
        let vol = volume as f32;
        *self.volume.lock().unwrap() = vol;
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.set_volume(vol);
            debug_log!("Volume set on sink: {}", volume);
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
        self.state.lock().unwrap().clone()
    }

    #[napi]
    pub fn get_duration(&self) -> Result<f64> {
        Ok(*self.duration.lock().unwrap())
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

// Factory functions

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

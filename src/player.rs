use base64::{engine::general_purpose, Engine as _};
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::{Decoder, OutputStreamBuilder, Sink, Source};
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
    // OutputStream needs to be kept alive along with sink
    #[allow(dead_code)]
    output_stream: Arc<Mutex<Option<rodio::OutputStream>>>,
    audio_buffer: Arc<Mutex<Option<Vec<u8>>>>,
    // Track if player was ever initialized
    initialized: bool,
    // Track current playback time (using Unix timestamp for napi compatibility)
    start_time: Arc<Mutex<Option<u128>>>,
    total_paused_ns: Arc<Mutex<u128>>,
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
            start_time: Arc::new(Mutex::new(None)),
            total_paused_ns: Arc::new(Mutex::new(0)),
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
        let player = Self::default();

        // Try to initialize the output stream and sink immediately
        // This prevents the first-play delay
        match OutputStreamBuilder::open_default_stream() {
            Ok(stream) => {
                let sink = Sink::connect_new(stream.mixer());
                *player.output_stream.lock().unwrap() = Some(stream);
                *player.sink.lock().unwrap() = Some(sink);
                debug_log!("Audio stream initialized in constructor");
            }
            Err(e) => {
                debug_log!("Failed to open default audio output in constructor: {}", e);
            }
        }

        Ok(player)
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

        debug_log!(
            "Play called, current state: {:?}",
            self.state.lock().unwrap().clone()
        );

        // Track playback time
        {
            let mut start_time_guard = self.start_time.lock().unwrap();
            let mut total_paused_guard = self.total_paused_ns.lock().unwrap();
            let current_state = self.state.lock().unwrap().clone();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_nanos();

            if current_state == PlaybackState::Paused {
                // Resuming from pause - calculate how long we were paused
                if let Some(pause_start_ns) = *start_time_guard {
                    let paused_ns = now.saturating_sub(pause_start_ns);
                    *total_paused_guard = total_paused_guard.saturating_add(paused_ns);
                }
            } else {
                // Fresh start - reset everything
                *total_paused_guard = 0;
            }
            *start_time_guard = Some(now);
        }

        // Always ensure sink is available - recreate if needed
        let sink_needs_source = {
            let mut output_stream_guard = self.output_stream.lock().unwrap();
            let mut sink_guard = self.sink.lock().unwrap();

            // Create new stream and sink if either is missing
            if sink_guard.is_none() || output_stream_guard.is_none() {
                debug_log!("Recreating output stream and sink...");

                let stream = OutputStreamBuilder::open_default_stream().map_err(|e| {
                    debug_log!("Failed to create output stream: {}", e);
                    Error::new(
                        Status::GenericFailure,
                        format!("Failed to create output stream: {}", e),
                    )
                })?;

                let sink = Sink::connect_new(stream.mixer());

                *output_stream_guard = Some(stream);
                *sink_guard = Some(sink);
                debug_log!("Output stream and sink recreated");
                true // New sink needs a source
            } else {
                // Sink exists, check if it needs a source
                sink_guard.as_ref().map(|s| s.empty()).unwrap_or(true)
            }
        };

        // Append source and play
        let sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_ref() {
            let volume = *self.volume.lock().unwrap();
            sink.set_volume(volume);
            debug_log!("Setting volume to: {}", volume);

            if sink_needs_source || sink.empty() {
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
            } else {
                debug_log!("Resuming paused audio");
            }
            sink.play();
            debug_log!("Sink playing");
        }

        *self.state.lock().unwrap() = PlaybackState::Playing;
        debug_log!("State set to Playing");

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

        // Track pause start time
        {
            let mut start_time_guard = self.start_time.lock().unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_nanos();
            *start_time_guard = Some(now);
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

        // Reset time tracking
        {
            let mut start_time_guard = self.start_time.lock().unwrap();
            let mut total_paused_guard = self.total_paused_ns.lock().unwrap();
            *start_time_guard = None;
            *total_paused_guard = 0;
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
        let start_time_opt = *self.start_time.lock().unwrap();

        if let Some(start_time_ns) = start_time_opt {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_nanos();
            let total_paused_ns = *self.total_paused_ns.lock().unwrap();
            let playing_ns = now
                .saturating_sub(start_time_ns)
                .saturating_sub(total_paused_ns);
            Ok(playing_ns as f64 / 1_000_000_000.0)
        } else {
            Ok(0.0)
        }
    }

    #[napi]
    pub fn get_current_file(&self) -> Option<String> {
        self.current_file.clone()
    }

    #[napi]
    pub fn seek_to(&mut self, position: f64) -> Result<()> {
        debug_log!("Seek to position: {} seconds", position);

        // Validate position - handle decimal precision issues
        if position.is_nan() || position.is_infinite() {
            return Err(Error::new(
                Status::InvalidArg,
                "Position must be a valid finite number",
            ));
        }

        let duration = *self.duration.lock().unwrap();
        // Use a small epsilon for floating point comparison
        let epsilon = 1e-9;
        if position < -epsilon || position > duration + epsilon {
            return Err(Error::new(
                Status::InvalidArg,
                format!("Position must be between 0.0 and {} seconds", duration),
            ));
        }

        // Clamp position to valid range
        let position = position.max(0.0).min(duration);

        // Check if we have a source to seek within
        let has_file = self.current_file.is_some();
        let has_buffer = self.audio_buffer.lock().unwrap().is_some();

        if !has_file && !has_buffer {
            debug_log!("Seek called but no audio loaded");
            return Err(Error::new(Status::InvalidArg, "No audio loaded"));
        }

        // Stop current playback without clearing source info
        {
            let sink_guard = self.sink.lock().unwrap();
            if let Some(sink) = sink_guard.as_ref() {
                sink.stop();
            }
        }
        *self.sink.lock().unwrap() = None;
        *self.state.lock().unwrap() = PlaybackState::Stopped;
        debug_log!("Sink stopped for seek");

        // Reset time tracking for new playback position
        {
            let mut start_time_guard = self.start_time.lock().unwrap();
            let mut total_paused_guard = self.total_paused_ns.lock().unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_nanos();

            // Calculate the effective start time (now minus the seek position)
            // Use saturating arithmetic to prevent underflow
            let seek_position_ns = (position * 1_000_000_000.0) as u128;
            *start_time_guard = Some(now.saturating_sub(seek_position_ns));
            *total_paused_guard = 0;
        }

        // Recreate output stream and sink only if needed
        {
            let output_stream_guard = self.output_stream.lock().unwrap();
            let sink_guard = self.sink.lock().unwrap();

            if sink_guard.is_none() || output_stream_guard.is_none() {
                drop(sink_guard);
                drop(output_stream_guard);

                let stream = OutputStreamBuilder::open_default_stream().map_err(|e| {
                    debug_log!("Failed to create output stream for seek: {}", e);
                    Error::new(
                        Status::GenericFailure,
                        format!("Failed to create output stream: {}", e),
                    )
                })?;

                let sink_new = Sink::connect_new(stream.mixer());
                *self.output_stream.lock().unwrap() = Some(stream);
                *self.sink.lock().unwrap() = Some(sink_new);
                debug_log!("Output stream and sink recreated for seek");
            }
        }

        // Create source with skip and append to sink
        let sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_ref() {
            let volume = *self.volume.lock().unwrap();
            sink.set_volume(volume);

            if let Some(ref file_path) = self.current_file {
                let path = Path::new(file_path);
                let file = File::open(path).map_err(|e| {
                    Error::new(
                        Status::GenericFailure,
                        format!("Failed to reopen file: {}", e),
                    )
                })?;

                let reader = BufReader::new(file);
                let decoder = Decoder::new(reader).map_err(|e| {
                    Error::new(
                        Status::GenericFailure,
                        format!("Failed to create decoder: {}", e),
                    )
                })?;

                // Skip to the desired position
                let skip_duration = std::time::Duration::from_secs_f64(position);
                let source = decoder.skip_duration(skip_duration);
                sink.append(source);
                debug_log!("File source appended with skip to position: {}s", position);
            } else if let Some(ref buffer_data) = *self.audio_buffer.lock().unwrap() {
                // For buffer sources, we skip bytes based on approximate position
                let sample_rate = 44100.0; // Assume common sample rate
                let bytes_per_second = sample_rate * 4.0; // 16-bit stereo = 4 bytes per sample
                let skip_bytes = ((position * bytes_per_second) as usize).min(buffer_data.len());

                let cursor = Cursor::new(buffer_data[skip_bytes..].to_vec());
                let decoder = Decoder::new(cursor).map_err(|e| {
                    Error::new(
                        Status::GenericFailure,
                        format!("Failed to create decoder: {}", e),
                    )
                })?;

                sink.append(decoder);
                debug_log!(
                    "Buffer source appended with skip to position: {}s",
                    position
                );
            }

            sink.play();
            *self.state.lock().unwrap() = PlaybackState::Playing;
            debug_log!("Seek complete, playing from position: {}s", position);
        }

        Ok(())
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

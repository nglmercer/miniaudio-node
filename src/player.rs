use base64::{engine::general_purpose, Engine as _};
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;
use std::sync::{Arc, Mutex};

// Importamos los tipos definidos en el otro módulo
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
        let path = Path::new(&file_path);
        if !path.exists() {
            return Err(Error::new(
                Status::InvalidArg,
                format!("File not found: {}", file_path),
            ));
        }

        self.stop().ok();

        // Validate file opening
        let file = File::open(path)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to open file: {}", e)))?;
        let reader = BufReader::new(file);
        let _decoder = Decoder::new(reader).map_err(|e| {
            Error::new(
                Status::InvalidArg,
                format!("Failed to create decoder: {}", e),
            )
        })?;

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

        Ok(())
    }

    #[napi]
    pub fn load_base64(&mut self, base64_data: String) -> Result<()> {
        if base64_data.is_empty() {
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
        let current_state = *self.state.lock().unwrap();
        if current_state != PlaybackState::Playing {
            let has_buffer = self.audio_buffer.lock().unwrap().is_some();
            let has_file = self.current_file.is_some();

            if !has_buffer && !has_file {
                return Err(Error::new(Status::InvalidArg, "Player not initialized"));
            }

            let mut output_stream_guard = self.output_stream.lock().unwrap();
            if output_stream_guard.is_none() {
                let stream = OutputStreamBuilder::open_default_stream().map_err(|e| {
                    Error::new(
                        Status::GenericFailure,
                        format!("Failed to create output stream: {}", e),
                    )
                })?;

                // We keep the stream alive
                *output_stream_guard = Some(stream);

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
                sink.set_volume(*self.volume.lock().unwrap());

                if sink.empty() {
                    if let Some(buffer_data) = self.audio_buffer.lock().unwrap().clone() {
                        let cursor = Cursor::new(buffer_data);
                        let source = Decoder::new(cursor).unwrap();
                        sink.append(source);
                    } else if let Some(file_path) = &self.current_file {
                        let file = File::open(file_path).unwrap();
                        let source = Decoder::new(BufReader::new(file)).unwrap();
                        sink.append(source);
                    }
                    sink.play();
                } else {
                    sink.play(); // Resume if paused
                }
            }

            *self.state.lock().unwrap() = PlaybackState::Playing;
        }
        Ok(())
    }

    #[napi]
    pub fn pause(&mut self) -> Result<()> {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.pause();
            *self.state.lock().unwrap() = PlaybackState::Paused;
        }
        Ok(())
    }

    #[napi]
    pub fn stop(&mut self) -> Result<()> {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            sink.stop();
        }
        // Re-create sink logic might be needed here depending on rodio version behavior,
        // usually stop() kills the sink.
        *self.sink.lock().unwrap() = None;
        *self.audio_buffer.lock().unwrap() = None;
        *self.state.lock().unwrap() = PlaybackState::Stopped;
        self.current_file = None;
        Ok(())
    }

    #[napi]
    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        let vol = volume.max(0.0).min(1.0) as f32;
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

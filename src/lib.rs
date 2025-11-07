#[macro_use]
extern crate napi_derive;

use napi::bindgen_prelude::*;
use napi::Result as NapiResult;
use std::sync::{Arc, Mutex};
use std::path::Path;
use rodio::{OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;

#[napi(object)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[napi]
pub struct AudioPlayer {
    _stream: Option<OutputStream>,
    sink: Option<Arc<Mutex<Sink>>>,
    current_file: Option<String>,
    volume: f32,
}

#[napi]
impl Default for AudioPlayer {
    fn default() -> Self {
        Self {
            _stream: None,
            sink: None,
            current_file: None,
            volume: 1.0,
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
    pub fn get_devices(&self) -> NapiResult<Vec<AudioDeviceInfo>> {
        // Rodio doesn't provide direct device enumeration
        // We'll return a default device for now
        let devices = vec![
            AudioDeviceInfo {
                id: "0".to_string(),
                name: "Default Output Device".to_string(),
                is_default: true,
            }
        ];
        Ok(devices)
    }

    #[napi]
    pub fn load_file(&mut self, file_path: String) -> NapiResult<()> {
        // Validate file exists
        if !Path::new(&file_path).exists() {
            return Err(Error::new(
                Status::InvalidArg,
                format!("Audio file does not exist: {}", file_path),
            ));
        }

        // Clean up existing resources
        if let Some(sink) = &self.sink {
            let sink = sink.lock().unwrap();
            sink.stop();
            sink.clear();
        }
        self.sink = None;

        // Store the file path for later playback
        self.current_file = Some(file_path.clone());

        // Initialize output stream if not already done
        if self._stream.is_none() {
            let (stream, stream_handle) = OutputStream::try_default()
                .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to create output stream: {}", e)))?;

            // Create sink for playback using the handle
            let sink = Sink::try_new(&stream_handle)
                .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to create sink: {}", e)))?;

            sink.set_volume(self.volume);
            self.sink = Some(Arc::new(Mutex::new(sink)));

            // Keep the stream alive
            self._stream = Some(stream);
        }

        Ok(())
    }

    #[napi]
    pub fn play(&mut self) -> NapiResult<()> {
        if let (Some(sink), Some(file_path)) = (&self.sink, &self.current_file) {
            // Load the audio file fresh
            let file = File::open(Path::new(file_path))
                .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to open file: {}", e)))?;

            let source = rodio::Decoder::new(BufReader::new(file))
                .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to create decoder: {}", e)))?;

            let sink = sink.lock().unwrap();

            // Clear any existing audio and add new
            sink.clear();
            sink.append(source);
            sink.play();
        } else {
            return Err(Error::new(Status::InvalidArg, "No audio file loaded. Call load_file() first.".to_string()));
        }

        Ok(())
    }

    #[napi]
    pub fn pause(&mut self) -> NapiResult<()> {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().pause();
        } else {
            return Err(Error::new(Status::InvalidArg, "No audio player initialized".to_string()));
        }

        Ok(())
    }

    #[napi]
    pub fn stop(&mut self) -> NapiResult<()> {
        if let Some(sink) = &self.sink {
            let sink = sink.lock().unwrap();
            sink.stop();
            sink.clear();
        } else {
            return Err(Error::new(Status::InvalidArg, "No audio player initialized".to_string()));
        }

        Ok(())
    }

    #[napi]
    pub fn set_volume(&mut self, volume: f64) -> NapiResult<()> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(Error::new(Status::InvalidArg, "Volume must be between 0.0 and 1.0".to_string()));
        }

        self.volume = volume as f32;

        if let Some(sink) = &self.sink {
            sink.lock().unwrap().set_volume(self.volume);
        }

        Ok(())
    }

    #[napi]
    pub fn get_volume(&self) -> NapiResult<f64> {
        Ok(self.volume as f64)
    }

    #[napi]
    pub fn is_playing(&self) -> bool {
        if let Some(sink) = &self.sink {
            let sink = sink.lock().unwrap();
            !sink.is_paused() && !sink.empty()
        } else {
            false
        }
    }

    #[napi]
    pub fn get_duration(&self) -> NapiResult<f64> {
        // Rodio doesn't provide direct duration information
        // This would require additional metadata library
        // TODO: Implement with metadata library
        Ok(0.0)
    }

    #[napi]
    pub fn get_current_time(&self) -> NapiResult<f64> {
        // Rodio doesn't provide direct position information
        // This would require additional tracking
        // TODO: Implement position tracking
        Ok(0.0)
    }
}

#[napi]
pub fn get_supported_formats() -> Vec<String> {
    vec![
        "wav".to_string(),
        "mp3".to_string(),
        "flac".to_string(),
        "ogg".to_string(),
    ]
}

#[napi]
pub fn initialize_audio() -> NapiResult<String> {
    // Rodio initializes on-demand, so we just verify we can create an output stream
    match OutputStream::try_default() {
        Ok(_) => Ok("Audio system initialized successfully".to_string()),
        Err(e) => Err(Error::new(Status::GenericFailure, format!("Failed to initialize audio: {}", e))),
    }
}

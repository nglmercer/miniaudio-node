use base64::{engine::general_purpose, Engine as _};
use cpal::traits::{DeviceTrait, HostTrait};
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

const DEVICE_ID_SEPARATOR: char = ':';

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
    // Decoded PCM samples (f32, rodio's native sample type) for content
    // loaded via loadBuffer/loadBase64. Storing decoded samples (instead of
    // the encoded container bytes) makes duration reporting and seeking
    // correct for any supported format.
    audio_samples: Arc<Mutex<Option<Vec<f32>>>>,
    // Real sample rate / channel count of the loaded buffer content.
    // Required to correctly seek within buffer sources (TTS audio is often
    // mono and not 44100 Hz, e.g. 22050 Hz pcm_s16le).
    buffer_sample_rate: u32,
    buffer_channels: u16,
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
            audio_samples: Arc::new(Mutex::new(None)),
            buffer_sample_rate: 0,
            buffer_channels: 0,
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
                *player.output_stream.lock().unwrap_or_else(|e| e.into_inner()) = Some(stream);
                *player.sink.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink);
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
        let mut result = Vec::new();

        for host_id in cpal::available_hosts() {
            let host = match cpal::host_from_id(host_id) {
                Ok(h) => h,
                Err(_) => continue,
            };

            let host_name = format!("{:?}", host_id);
            let devices = match host.output_devices() {
                Ok(d) => d,
                Err(_) => continue,
            };

            let default_name = host
                .default_output_device()
                .and_then(|d| d.description().ok().map(|desc| desc.name().to_string()));

            for (i, device) in devices.enumerate() {
                let name = match device.description() {
                    Ok(desc) => desc.name().to_string(),
                    Err(_) => continue,
                };
                result.push(AudioDeviceInfo {
                    id: format!("{}{}{}", host_name, DEVICE_ID_SEPARATOR, i),
                    name: name.clone(),
                    host: host_name.clone(),
                    is_default: Some(name) == default_name,
                });
            }
        }

        // Fallback so callers always get at least one entry even when
        // enumeration fails (e.g. no audio hardware in a container).
        if result.is_empty() {
            result.push(AudioDeviceInfo {
                id: "default".to_string(),
                name: "Default Output Device".to_string(),
                host: "default".to_string(),
                is_default: true,
            });
        }

        Ok(result)
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
        *self.duration.lock().unwrap_or_else(|e| e.into_inner()) = duration_seconds;

        debug_log!(
            "File loaded successfully, duration: {} seconds",
            duration_seconds
        );
        self.current_file = Some(file_path);
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = PlaybackState::Loaded;

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

        let cursor = Cursor::new(audio_data);
        let decoder = Decoder::new(cursor).map_err(|e| {
            Error::new(
                Status::InvalidArg,
                format!("Failed to decode buffer: {}", e),
            )
        })?;

        // Capture the real format so buffer seeks use correct offsets.
        let buffer_sample_rate = decoder.sample_rate();
        let buffer_channels = decoder.channels();

        // Decode to PCM up-front so duration and seeking are accurate for
        // any supported container (raw byte offsets into encoded data would
        // land mid-frame or past the header and fail to decode).
        let samples: Vec<f32> = decoder.collect();
        if samples.is_empty() {
            return Err(Error::new(
                Status::InvalidArg,
                "Buffer contained no decodable audio data",
            ));
        }

        let duration_seconds = samples.len() as f64
            / (buffer_sample_rate as f64 * buffer_channels.max(1) as f64);

        *self.duration.lock().unwrap_or_else(|e| e.into_inner()) = duration_seconds;
        *self.audio_samples.lock().unwrap_or_else(|e| e.into_inner()) = Some(samples);
        self.buffer_sample_rate = buffer_sample_rate;
        self.buffer_channels = buffer_channels;
        self.current_file = Some(format!(
            "__BUFFER__{}",
            std::time::SystemTime::now()
                .elapsed()
                .unwrap_or_default()
                .as_millis()
        ));
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = PlaybackState::Loaded;
        debug_log!(
            "Buffer loaded successfully, duration: {} seconds",
            duration_seconds
        );

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
        let has_buffer = self.audio_samples.lock().unwrap_or_else(|e| e.into_inner()).is_some();
        let has_file = self.current_file.is_some();

        if !has_buffer && !has_file {
            debug_log!("Play called but player not initialized");
            return Err(Error::new(Status::InvalidArg, "Player not initialized"));
        }

        debug_log!(
            "Play called, current state: {:?}",
            self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
        );

        // Track playback time
        {
            let mut start_time_guard = self.start_time.lock().unwrap_or_else(|e| e.into_inner());
            let mut total_paused_guard = self.total_paused_ns.lock().unwrap_or_else(|e| e.into_inner());
            let current_state = self.state.lock().unwrap_or_else(|e| e.into_inner()).clone();
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
            let mut output_stream_guard = self.output_stream.lock().unwrap_or_else(|e| e.into_inner());
            let mut sink_guard = self.sink.lock().unwrap_or_else(|e| e.into_inner());

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
        let sink_guard = self.sink.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sink) = sink_guard.as_ref() {
            let volume = *self.volume.lock().unwrap_or_else(|e| e.into_inner());
            sink.set_volume(volume);
            debug_log!("Setting volume to: {}", volume);

            if sink_needs_source || sink.empty() {
                debug_log!("Sink is empty, appending source...");
                if let Some(samples) = self.audio_samples.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                    debug_log!("Playing from buffer ({} samples)", samples.len());
                    let channels = if self.buffer_channels > 0 {
                        self.buffer_channels
                    } else {
                        2
                    };
                    let sample_rate = if self.buffer_sample_rate > 0 {
                        self.buffer_sample_rate
                    } else {
                        44100
                    };
                    let source =
                        rodio::buffer::SamplesBuffer::new(channels, sample_rate, samples);
                    sink.append(source);
                } else if let Some(file_path) = &self.current_file {
                    debug_log!("Playing from file: {}", file_path);
                    let file = File::open(file_path).map_err(|e| {
                        Error::new(
                            Status::GenericFailure,
                            format!("Failed to open file '{}': {}", file_path, e),
                        )
                    })?;
                    let source = Decoder::new(BufReader::new(file)).map_err(|e| {
                        Error::new(
                            Status::GenericFailure,
                            format!("Failed to decode file '{}': {}", file_path, e),
                        )
                    })?;
                    sink.append(source);
                }
            } else {
                debug_log!("Resuming paused audio");
            }
            sink.play();
            debug_log!("Sink playing");
        }

        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = PlaybackState::Playing;
        debug_log!("State set to Playing");

        Ok(())
    }

    #[napi]
    pub fn pause(&mut self) -> Result<()> {
        debug_log!("Pause called");
        let current_state = self.state.lock().unwrap_or_else(|e| e.into_inner()).clone();

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
            let mut start_time_guard = self.start_time.lock().unwrap_or_else(|e| e.into_inner());
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_nanos();
            *start_time_guard = Some(now);
        }

        let sink_guard = self.sink.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sink) = sink_guard.as_ref() {
            sink.pause();
            *self.state.lock().unwrap_or_else(|e| e.into_inner()) = PlaybackState::Paused;
            debug_log!("State set to Paused");
            Ok(())
        } else {
            // Sink doesn't exist but player is in Playing/Loaded state
            // This can happen after stop() was called but before play()
            // Just update state to Paused since there's nothing playing
            *self.state.lock().unwrap_or_else(|e| e.into_inner()) = PlaybackState::Paused;
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
            let mut start_time_guard = self.start_time.lock().unwrap_or_else(|e| e.into_inner());
            let mut total_paused_guard = self.total_paused_ns.lock().unwrap_or_else(|e| e.into_inner());
            *start_time_guard = None;
            *total_paused_guard = 0;
        }

        if let Some(sink) = self.sink.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            debug_log!("Stopping sink");
            sink.stop();
        }
        *self.sink.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.audio_samples.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.buffer_sample_rate = 0;
        self.buffer_channels = 0;
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = PlaybackState::Stopped;
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
        *self.volume.lock().unwrap_or_else(|e| e.into_inner()) = vol;
        if let Some(sink) = self.sink.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            sink.set_volume(vol);
            debug_log!("Volume set on sink: {}", volume);
        }
        Ok(())
    }

    #[napi]
    pub fn get_volume(&self) -> Result<f64> {
        Ok(*self.volume.lock().unwrap_or_else(|e| e.into_inner()) as f64)
    }

    #[napi]
    pub fn is_playing(&self) -> bool {
        if let Some(sink) = self.sink.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            !sink.is_paused() && !sink.empty()
        } else {
            false
        }
    }

    #[napi]
    pub fn get_state(&self) -> PlaybackState {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    #[napi]
    pub fn get_duration(&self) -> Result<f64> {
        Ok(*self.duration.lock().unwrap_or_else(|e| e.into_inner()))
    }

    #[napi]
    pub fn get_current_time(&self) -> Result<f64> {
        let start_time_opt = *self.start_time.lock().unwrap_or_else(|e| e.into_inner());

        if let Some(start_time_ns) = start_time_opt {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_nanos();
            let total_paused_ns = *self.total_paused_ns.lock().unwrap_or_else(|e| e.into_inner());
            let playing_ns = now
                .saturating_sub(start_time_ns)
                .saturating_sub(total_paused_ns);
            let mut current = playing_ns as f64 / 1_000_000_000.0;
            // Clamp to duration so the clock stops when the track ends.
            let duration = *self.duration.lock().unwrap_or_else(|e| e.into_inner());
            if duration > 0.0 && current > duration {
                current = duration;
            }
            Ok(current)
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

        let duration = *self.duration.lock().unwrap_or_else(|e| e.into_inner());
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
        let has_buffer = self.audio_samples.lock().unwrap_or_else(|e| e.into_inner()).is_some();

        if !has_file && !has_buffer {
            debug_log!("Seek called but no audio loaded");
            return Err(Error::new(Status::InvalidArg, "No audio loaded"));
        }

        // Stop current playback without clearing source info
        {
            let sink_guard = self.sink.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(sink) = sink_guard.as_ref() {
                sink.stop();
            }
        }
        *self.sink.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = PlaybackState::Stopped;
        debug_log!("Sink stopped for seek");

        // Reset time tracking for new playback position
        {
            let mut start_time_guard = self.start_time.lock().unwrap_or_else(|e| e.into_inner());
            let mut total_paused_guard = self.total_paused_ns.lock().unwrap_or_else(|e| e.into_inner());
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
            let output_stream_guard = self.output_stream.lock().unwrap_or_else(|e| e.into_inner());
            let sink_guard = self.sink.lock().unwrap_or_else(|e| e.into_inner());

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
                *self.output_stream.lock().unwrap_or_else(|e| e.into_inner()) = Some(stream);
                *self.sink.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink_new);
                debug_log!("Output stream and sink recreated for seek");
            }
        }

        // Create source with skip and append to sink
        let sink_guard = self.sink.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sink) = sink_guard.as_ref() {
            let volume = *self.volume.lock().unwrap_or_else(|e| e.into_inner());
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
            } else if let Some(samples) = self.audio_samples.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                // Buffer content is stored as decoded PCM, so seeking is a
                // simple sample-offset skip using the real format of the
                // loaded content (TTS audio is commonly mono and not
                // 44100 Hz, e.g. 22050 Hz pcm_s16le).
                let channels = if self.buffer_channels > 0 {
                    self.buffer_channels
                } else {
                    2
                };
                let sample_rate = if self.buffer_sample_rate > 0 {
                    self.buffer_sample_rate
                } else {
                    44100
                };
                let skip_samples =
                    buffer_skip_samples(sample_rate, channels, position, samples.len());

                let source = rodio::buffer::SamplesBuffer::new(
                    channels,
                    sample_rate,
                    samples[skip_samples..].to_vec(),
                );
                sink.append(source);
                debug_log!(
                    "Buffer source appended with skip to position: {}s",
                    position
                );
            }

            sink.play();
            *self.state.lock().unwrap_or_else(|e| e.into_inner()) = PlaybackState::Playing;
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

/// Compute the interleaved-sample offset to skip when seeking within decoded
/// PCM buffer data.
///
/// `sample_rate` and `channels` are the *real* format of the loaded audio.
/// Using incorrect values (the previous hard-coded `44100 Hz` / stereo) makes
/// seeking TTS-generated audio (commonly mono, e.g. 22050 Hz) land at the
/// wrong position or skip past the end of the data entirely.
fn buffer_skip_samples(sample_rate: u32, channels: u16, position: f64, samples_len: usize) -> usize {
    let channels = (channels as u64).max(1);
    let samples_per_second = channels * sample_rate as u64;
    let skip = (position * samples_per_second as f64) as usize;
    skip.min(samples_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_skip_samples_mono_22050_tts() {
        // TTS output: mono, 22050 Hz pcm_s16le.
        let sample_rate = 22050u32;
        let channels = 1u16;
        let samples_len = 22050; // 1s of mono audio

        // 0.5s * 22050 = 11025 samples
        assert_eq!(buffer_skip_samples(sample_rate, channels, 0.5, samples_len), 11025);

        // 1.0s * 22050 = 22050 samples
        assert_eq!(buffer_skip_samples(sample_rate, channels, 1.0, samples_len), 22050);

        // Seeking past the end is clamped to the sample count.
        assert_eq!(
            buffer_skip_samples(sample_rate, channels, 100.0, samples_len),
            samples_len
        );
    }

    #[test]
    fn test_buffer_skip_samples_stereo_44100() {
        let sample_rate = 44100u32;
        let channels = 2u16;

        // 1.0s * 44100 * 2 = 88200 interleaved samples
        assert_eq!(
            buffer_skip_samples(sample_rate, channels, 1.0, usize::MAX),
            88200
        );
    }

    #[test]
    fn test_buffer_skip_samples_zero_channels_falls_back_to_one() {
        // Guard against a buffer with 0 channels (should never happen).
        assert_eq!(buffer_skip_samples(22050, 0, 0.5, usize::MAX), 11025);
    }
}

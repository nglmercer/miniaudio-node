use base64::{engine::general_purpose, Engine as _};
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::cpal;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStreamBuilder, Sink, Source};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

// Import the types defined in the other module.
use crate::debug_log;
use crate::types::{AudioDeviceInfo, AudioPlayerConfig, PlaybackState};
use crate::utils::set_debug;

const DEVICE_ID_SEPARATOR: char = ':';

#[derive(Default)]
struct PlaybackClock {
    position: f64,
    interval_started_at: Option<Instant>,
}

impl PlaybackClock {
    fn start(&mut self, now: Instant, reset_position: bool) {
        if reset_position {
            self.position = 0.0;
        }
        self.interval_started_at = Some(now);
    }

    fn pause(&mut self, now: Instant) {
        if let Some(started_at) = self.interval_started_at.take() {
            self.position += now.duration_since(started_at).as_secs_f64();
        }
    }

    fn reset(&mut self) {
        self.position = 0.0;
        self.interval_started_at = None;
    }

    fn seek(&mut self, position: f64, now: Instant, playing: bool) {
        self.position = position;
        self.interval_started_at = playing.then_some(now);
    }

    fn current(&self, now: Instant, duration: f64) -> f64 {
        let elapsed = self
            .interval_started_at
            .map(|started_at| now.duration_since(started_at).as_secs_f64())
            .unwrap_or(0.0);
        (self.position + elapsed)
            .max(0.0)
            .min(if duration > 0.0 { duration } else { f64::MAX })
    }

    fn reached_end(&self, now: Instant, duration: f64) -> bool {
        duration.is_finite() && duration > 0.0 && self.current(now, duration) >= duration
    }
}

struct PlaybackStartPlan {
    starts_new_track: bool,
    starts_clock: bool,
    replace_queued_sink: bool,
    source_position: f64,
}

fn playback_start_plan(
    current_state: &PlaybackState,
    position_before_play: f64,
    sink_is_empty: bool,
) -> PlaybackStartPlan {
    let starts_new_track = *current_state == PlaybackState::Stopped
        || (*current_state == PlaybackState::Loaded && position_before_play <= f64::EPSILON)
        || (*current_state == PlaybackState::Playing && sink_is_empty);

    PlaybackStartPlan {
        starts_new_track,
        starts_clock: *current_state != PlaybackState::Playing || starts_new_track,
        replace_queued_sink: starts_new_track && !sink_is_empty,
        source_position: if *current_state == PlaybackState::Loaded && !starts_new_track {
            position_before_play
        } else {
            0.0
        },
    }
}

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
    // Playback position accumulated before the current playing interval.
    // Monotonic clock for the current playing interval. Keeping this separate
    // from the accumulated position makes pause/resume timing unambiguous.
    clock: Arc<Mutex<PlaybackClock>>,
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
            clock: Arc::new(Mutex::new(PlaybackClock::default())),
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
        // Open the backend lazily on the first play() call. Constructing a
        // player must remain safe in headless environments, where probing a
        // PulseAudio/CoreAudio device can block or fail before the caller has
        // requested playback.
        Ok(Self::default())
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

            let default_name = host.default_output_device().and_then(|d| d.name().ok());

            for (i, device) in devices.enumerate() {
                let name = match device.name() {
                    Ok(name) => name,
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

        let duration_seconds =
            samples.len() as f64 / (buffer_sample_rate as f64 * buffer_channels.max(1) as f64);

        *self.duration.lock().unwrap_or_else(|e| e.into_inner()) = duration_seconds;
        *self.audio_samples.lock().unwrap_or_else(|e| e.into_inner()) = Some(samples);
        self.buffer_sample_rate = buffer_sample_rate;
        self.buffer_channels = buffer_channels;
        self.current_file = Some(format!(
            "__BUFFER__{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
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
        let has_buffer = self
            .audio_samples
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some();
        let has_file = self.current_file.is_some();

        if !has_buffer && !has_file {
            debug_log!("Play called but player not initialized");
            return Err(Error::new(Status::InvalidArg, "Player not initialized"));
        }

        debug_log!(
            "Play called, current state: {:?}",
            self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
        );

        // Plan a fresh playback or resume. Calling play() while already
        // playing is intentionally idempotent.
        let current_state = self.reconcile_state();
        let position_before_play = self
            .clock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .position;
        let sink_is_empty = self
            .sink
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|sink| sink.empty())
            .unwrap_or(true);
        let start_plan = playback_start_plan(&current_state, position_before_play, sink_is_empty);

        // Always ensure sink is available - recreate if needed
        let sink_needs_source = {
            let mut output_stream_guard =
                self.output_stream.lock().unwrap_or_else(|e| e.into_inner());
            let mut sink_guard = self.sink.lock().unwrap_or_else(|e| e.into_inner());

            // A clock-forced EOF can leave Rodio's physical queue non-empty.
            // Every logical new-track start must replace that queue so replay
            // begins from the source's first sample instead of resuming it.
            if start_plan.replace_queued_sink {
                if let Some(sink) = sink_guard.as_ref() {
                    sink.stop();
                }
                *sink_guard = None;
            }

            if output_stream_guard.is_none() {
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
            } else if sink_guard.is_none() {
                let stream = output_stream_guard.as_ref().ok_or_else(|| {
                    Error::new(
                        Status::GenericFailure,
                        "Output stream disappeared while recreating the sink",
                    )
                })?;
                let sink = Sink::connect_new(stream.mixer());
                *sink_guard = Some(sink);
                debug_log!("Sink recreated on the existing output stream");
                true
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
                if let Some(samples) = self
                    .audio_samples
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
                {
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
                    let skip_samples = buffer_skip_samples(
                        sample_rate,
                        channels,
                        start_plan.source_position,
                        samples.len(),
                    );
                    let source = rodio::buffer::SamplesBuffer::new(
                        channels,
                        sample_rate,
                        samples[skip_samples..].to_vec(),
                    );
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
                    sink.append(source.skip_duration(std::time::Duration::from_secs_f64(
                        start_plan.source_position,
                    )));
                }
            } else {
                debug_log!("Resuming paused audio");
            }
            sink.play();
            debug_log!("Sink playing");
        }

        // Start timing only after all backend setup and source preparation has
        // completed and playback has actually been issued to the sink.
        if start_plan.starts_clock {
            self.clock
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .start(Instant::now(), start_plan.starts_new_track);
        }

        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = PlaybackState::Playing;
        debug_log!("State set to Playing");

        Ok(())
    }

    #[napi]
    pub fn pause(&mut self) -> Result<()> {
        debug_log!("Pause called");
        let current_state = self.reconcile_state();

        // If already paused or stopped with no sink, just update state
        if current_state == PlaybackState::Paused {
            debug_log!("Already paused, no action needed");
            return Ok(());
        }

        if current_state == PlaybackState::Stopped {
            debug_log!("Player is stopped, nothing to pause");
            return Ok(());
        }

        // Commit the elapsed part of the current playing interval and stop
        // that interval. The paused duration is therefore never subtracted
        // from the playback position on resume.
        if current_state == PlaybackState::Playing {
            self.clock
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pause(Instant::now());
        } else {
            self.clock
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .interval_started_at = None;
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

        // Preserve the public error for an unused player, but still release
        // any eagerly-created backend resources so Drop can shut down cleanly.
        let was_initialized = self.initialized;
        if !was_initialized {
            debug_log!("Cannot stop - player not initialized");
        }

        // Reset time tracking.
        self.clock.lock().unwrap_or_else(|e| e.into_inner()).reset();

        if let Some(sink) = self.sink.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            debug_log!("Stopping sink");
            sink.stop();
        }
        *self.sink.lock().unwrap_or_else(|e| e.into_inner()) = None;
        // Drop the output stream together with its sink. Keeping the stream
        // alive after stop() leaves the backend worker running in headless
        // environments and can prevent test/process shutdown.
        *self.output_stream.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.audio_samples.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.buffer_sample_rate = 0;
        self.buffer_channels = 0;
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = PlaybackState::Stopped;
        self.current_file = None;
        debug_log!("State set to Stopped");

        if !was_initialized {
            return Err(Error::new(Status::InvalidArg, "Player not initialized"));
        }
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
        self.reconcile_state() == PlaybackState::Playing
    }

    fn reconcile_state(&self) -> PlaybackState {
        let sink_empty = self
            .sink
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|sink| sink.empty())
            .unwrap_or(true);
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if *state == PlaybackState::Playing {
            let duration = *self.duration.lock().unwrap_or_else(|e| e.into_inner());
            let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
            // Some CoreAudio devices keep a finished rodio source queued when
            // output is unavailable or delayed, so `Sink::empty()` is not a
            // sufficient EOF signal on every host. The monotonic playback
            // clock is authoritative when the loaded source has a duration;
            // retain the sink check for sources without duration metadata.
            if sink_empty || clock.reached_end(Instant::now(), duration) {
                if duration.is_finite() && duration > 0.0 {
                    clock.position = duration;
                }
                clock.interval_started_at = None;
                *state = PlaybackState::Stopped;
            }
        }
        state.clone()
    }

    #[napi]
    pub fn get_state(&self) -> PlaybackState {
        self.reconcile_state()
    }

    #[napi]
    pub fn get_duration(&self) -> Result<f64> {
        Ok(*self.duration.lock().unwrap_or_else(|e| e.into_inner()))
    }

    #[napi]
    pub fn get_current_time(&self) -> Result<f64> {
        self.reconcile_state();
        let duration = *self.duration.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self
            .clock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .current(Instant::now(), duration))
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
        if position > u64::MAX as f64 {
            return Err(Error::new(
                Status::InvalidArg,
                "Position is too large to represent safely",
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
        let has_buffer = self
            .audio_samples
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some();

        if !has_file && !has_buffer {
            debug_log!("Seek called but no audio loaded");
            return Err(Error::new(Status::InvalidArg, "No audio loaded"));
        }

        // Reconcile EOF before capturing the pre-seek state. Seeking must
        // preserve whether the track was playing, paused, or merely loaded.
        let previous_state = self.reconcile_state();

        // Stop the old sink without clearing the loaded source information.
        if let Some(sink) = self.sink.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            sink.stop();
        }
        *self.sink.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.output_stream.lock().unwrap_or_else(|e| e.into_inner()) = None;

        let needs_output = matches!(
            previous_state,
            PlaybackState::Playing | PlaybackState::Paused
        );
        if needs_output {
            let stream = OutputStreamBuilder::open_default_stream().map_err(|e| {
                debug_log!("Failed to create output stream for seek: {}", e);
                Error::new(
                    Status::GenericFailure,
                    format!("Failed to create output stream: {}", e),
                )
            })?;
            let sink = Sink::connect_new(stream.mixer());
            let volume = *self.volume.lock().unwrap_or_else(|e| e.into_inner());
            sink.set_volume(volume);

            // Buffer-loaded players carry a synthetic "__BUFFER__..." file
            // identifier, so prefer their decoded PCM over reopening a path.
            if let Some(samples) = self
                .audio_samples
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
            {
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
            } else if let Some(ref file_path) = self.current_file {
                let file = File::open(Path::new(file_path)).map_err(|e| {
                    Error::new(
                        Status::GenericFailure,
                        format!("Failed to reopen file: {}", e),
                    )
                })?;
                let decoder = Decoder::new(BufReader::new(file)).map_err(|e| {
                    Error::new(
                        Status::GenericFailure,
                        format!("Failed to create decoder: {}", e),
                    )
                })?;
                sink.append(decoder.skip_duration(std::time::Duration::from_secs_f64(position)));
                debug_log!("File source appended with skip to position: {}s", position);
            }

            if previous_state == PlaybackState::Playing {
                sink.play();
            }
            *self.sink.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink);
            *self.output_stream.lock().unwrap_or_else(|e| e.into_inner()) = Some(stream);
        }

        self.clock.lock().unwrap_or_else(|e| e.into_inner()).seek(
            position,
            Instant::now(),
            previous_state == PlaybackState::Playing,
        );
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = previous_state;
        debug_log!("Seek complete at position: {}s", position);
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
        if let Some(debug) = cfg.debug {
            set_debug(debug);
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
        if let Some(debug) = cfg.debug {
            set_debug(debug);
        }
    }
    player.load_file(file_path)?;

    let auto_play = config.as_ref().and_then(|c| c.auto_play).unwrap_or(false);
    if auto_play {
        player.play()?;
    }
    Ok(player)
}

#[cfg(test)]
mod playback_clock_tests {
    use super::{playback_start_plan, PlaybackClock};
    use crate::types::PlaybackState;
    use std::time::{Duration, Instant};

    #[test]
    fn playback_clock_preserves_position_across_pause_and_resume() {
        let base = Instant::now();
        let mut clock = PlaybackClock::default();

        clock.start(base, true);
        clock.pause(base + Duration::from_secs(10));
        assert!((clock.current(base + Duration::from_secs(15), 60.0) - 10.0).abs() < 1e-9);

        clock.start(base + Duration::from_secs(15), false);
        assert!((clock.current(base + Duration::from_secs(20), 60.0) - 15.0).abs() < 1e-9);
    }

    #[test]
    fn playback_clock_clamps_to_duration() {
        let base = Instant::now();
        let mut clock = PlaybackClock::default();
        clock.start(base, true);
        assert_eq!(clock.current(base + Duration::from_secs(20), 5.0), 5.0);
    }

    #[test]
    fn playback_clock_detects_end_without_sink_state() {
        let base = Instant::now();
        let mut clock = PlaybackClock::default();
        clock.start(base, true);
        assert!(!clock.reached_end(base + Duration::from_millis(79), 0.08));
        assert!(clock.reached_end(base + Duration::from_millis(81), 0.08));
    }

    #[test]
    fn stopped_replay_replaces_a_nonempty_sink_and_starts_from_zero() {
        let plan = playback_start_plan(&PlaybackState::Stopped, 0.08, false);

        assert!(plan.starts_new_track);
        assert!(plan.starts_clock);
        assert!(plan.replace_queued_sink);
        assert_eq!(plan.source_position, 0.0);
    }
}

/// Compute the interleaved-sample offset to skip when seeking within decoded
/// PCM buffer data.
///
/// `sample_rate` and `channels` are the *real* format of the loaded audio.
/// Using incorrect values (the previous hard-coded `44100 Hz` / stereo) makes
/// seeking TTS-generated audio (commonly mono, e.g. 22050 Hz) land at the
/// wrong position or skip past the end of the data entirely.
fn buffer_skip_samples(
    sample_rate: u32,
    channels: u16,
    position: f64,
    samples_len: usize,
) -> usize {
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
        assert_eq!(
            buffer_skip_samples(sample_rate, channels, 0.5, samples_len),
            11025
        );

        // 1.0s * 22050 = 22050 samples
        assert_eq!(
            buffer_skip_samples(sample_rate, channels, 1.0, samples_len),
            22050
        );

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

use crate::types::{AudioMetadata, DEBUG_ENABLED};
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::cpal;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStreamBuilder, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[napi]
pub fn initialize_audio() -> Result<String> {
    // Device discovery is enough to validate availability. Opening a rodio
    // stream here makes this otherwise lightweight API block indefinitely on
    // headless PulseAudio/CoreAudio hosts; playback opens the stream lazily.
    let host = cpal::default_host();
    let host_name = format!("{:?}", host.id());
    let output_device = host.default_output_device().ok_or_else(|| {
        Error::new(
            Status::GenericFailure,
            format!("No default audio output device found for {}", host_name),
        )
    })?;
    output_device.default_output_config().map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!(
                "Default audio output is unavailable for {}: {}",
                host_name, e
            ),
        )
    })?;

    Ok(format!(
        "Audio system initialized with rodio ({})",
        host_name
    ))
}

/// Enable or disable debug logging (defaults to false)
#[napi]
pub fn set_debug(enabled: bool) {
    DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Get current debug logging state
#[napi]
pub fn is_debug_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
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
pub fn is_format_supported(format: String) -> bool {
    get_supported_formats().contains(&format.to_lowercase())
}

#[napi]
pub fn get_audio_info() -> Result<String> {
    let host = cpal::default_host();
    let host_name = format!("{:?}", host.id());
    let device = host.default_output_device().ok_or_else(|| {
        Error::new(
            Status::GenericFailure,
            format!("No default audio output device found for {}", host_name),
        )
    })?;
    let device_name = device
        .name()
        .unwrap_or_else(|_| "<unnamed output device>".to_string());
    let config = device.default_output_config().map_err(|error| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to query default output device: {}", error),
        )
    })?;

    Ok(format!(
        "Audio system: rodio\nHost: {}\nDefault device: {}\nChannels: {}\nSample rate: {}\nSample format: {:?}",
        host_name,
        device_name,
        config.channels(),
        config.sample_rate().0,
        config.sample_format()
    ))
}

/// Start a sine-wave test tone without blocking the JavaScript thread.
#[napi]
pub fn test_tone(frequency: f64, duration_ms: u32) -> Result<()> {
    use rodio::source::SineWave;
    use std::sync::mpsc;

    let (ready_tx, ready_rx) = mpsc::channel();

    // Keep the CoreAudio stream on the worker that creates it. The stream is
    // deliberately never moved into a second thread after construction.
    std::thread::spawn(move || {
        let stream = match OutputStreamBuilder::open_default_stream() {
            Ok(stream) => stream,
            Err(error) => {
                let _ = ready_tx.send(Err(error.to_string()));
                return;
            }
        };
        let sink = Sink::connect_new(stream.mixer());
        let source = SineWave::new(frequency as f32)
            .take_duration(Duration::from_millis(duration_ms as u64))
            .amplify(0.3);
        sink.append(source);
        let _ = ready_tx.send(Ok(()));
        sink.sleep_until_end();
        drop(stream);
    });

    ready_rx
        .recv()
        .map_err(|_| {
            Error::new(
                Status::GenericFailure,
                "Audio playback worker exited before starting",
            )
        })?
        .map_err(|error| Error::new(Status::GenericFailure, error))
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

    // Decode the file to obtain the real duration.
    let file = File::open(path).map_err(|e| {
        Error::new(
            Status::InvalidArg,
            format!("Failed to open file '{}': {}", file_path, e),
        )
    })?;
    let decoder = Decoder::new(BufReader::new(file)).map_err(|e| {
        Error::new(
            Status::InvalidArg,
            format!("Failed to decode file '{}': {}", file_path, e),
        )
    })?;

    // Some formats (e.g. OGG Vorbis) may not report a total duration
    // without a full decode; fall back to 0.0 in that case.
    let duration = decoder.total_duration().unwrap_or(Duration::ZERO);
    let duration_seconds =
        duration.as_secs() as f64 + duration.subsec_nanos() as f64 / 1_000_000_000.0;

    // Note: tag extraction (title/artist/album) is not yet supported by the
    // underlying rodio decoder and is returned as None.
    Ok(AudioMetadata {
        duration: duration_seconds,
        title: None,
        artist: None,
        album: None,
    })
}

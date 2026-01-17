use crate::types::AudioMetadata;
use napi::{Error, Result, Status};
use napi_derive::napi;
use rodio::{OutputStreamBuilder, Sink, Source};
use std::path::Path;
use std::thread;
use std::time::Duration;

#[napi]
pub fn initialize_audio() -> Result<String> {
    match OutputStreamBuilder::open_default_stream() {
        Ok(_stream) => Ok("Audio system initialized with rodio".to_string()),
        Err(e) => Err(Error::new(
            Status::GenericFailure,
            format!("Failed to initialize audio: {}", e),
        )),
    }
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
    ]
}

#[napi]
pub fn is_format_supported(format: String) -> bool {
    get_supported_formats().contains(&format.to_lowercase())
}

#[napi]
pub fn get_audio_info() -> Result<String> {
    Ok("Audio system: rodio\nDefault device: Default Output Device\nChannels: Stereo\nSample rate: 44100".to_string())
}

#[napi]
pub fn test_tone(frequency: f64, duration_ms: u32) -> Result<()> {
    use rodio::source::SineWave;

    let stream = OutputStreamBuilder::open_default_stream().map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to create stream: {}", e),
        )
    })?;

    let mixer = stream.mixer();
    let sink = Sink::connect_new(mixer);

    let source = SineWave::new(frequency as f32)
        .take_duration(Duration::from_millis(duration_ms as u64))
        .amplify(0.3);

    sink.append(source);
    thread::sleep(Duration::from_millis(duration_ms as u64));

    Ok(())
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
    // Rodio basic placeholder
    Ok(AudioMetadata {
        duration: 0.0,
        title: None,
        artist: None,
        album: None,
    })
}

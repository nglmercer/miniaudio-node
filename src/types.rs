use napi_derive::napi;
use std::sync::atomic::AtomicBool;

/// Audio sample type (16-bit signed integer)
pub type Sample = i16;

/// Audio sample rate (typically 44100 or 48000 Hz)
pub type SampleRate = u32;

/// Audio channel count (1 = mono, 2 = stereo, etc.)
pub type ChannelCount = u16;

/// List of available input devices
pub type InputDevices = Vec<AudioDeviceInfo>;

/// List of available output devices
pub type OutputDevices = Vec<AudioDeviceInfo>;

/// Audio device information structure
#[napi(object)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub host: String,
    pub is_default: bool,
}

/// Audio player state enumeration
/// Audio player state enumeration
#[napi(string_enum)]
#[derive(Debug, PartialEq, Clone)]
pub enum PlaybackState {
    Stopped,
    Loaded,
    Playing,
    Paused,
}

#[napi(object)]
pub struct AudioPlayerConfig {
    pub volume: Option<f64>,
    pub auto_play: Option<bool>,
    pub debug: Option<bool>,
}

#[napi(object)]
pub struct AudioMetadata {
    pub duration: f64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

/// Error types for device operations
#[napi(string_enum)]
pub enum DevicesError {
    NoDevicesFound,
    PermissionDenied,
    InvalidDevice,
    NotInitialized,
}

/// Audio source function types (for generator sources)
#[napi(string_enum)]
pub enum SourceFunction {
    Sine,
    Square,
    Sawtooth,
    Triangle,
    WhiteNoise,
    PinkNoise,
    BrownNoise,
}

/// Error types for stream operations
#[napi(string_enum)]
pub enum StreamError {
    NotPlaying,
    EndOfFile,
    InvalidData,
    UnsupportedFormat,
}

/// Play error for stream operations
#[napi(string_enum)]
pub enum PlayError {
    AlreadyPlaying,
    NotLoaded,
    SystemError,
}

/// Seek error types
#[napi(string_enum)]
pub enum SeekError {
    InvalidPosition,
    NotSeekable,
    OutOfBounds,
}

/// Decoder error types (for audio decoding operations)
#[napi(string_enum)]
pub enum DecoderError {
    InvalidFormat,
    CorruptedData,
    UnsupportedCodec,
    IoError,
}

/// Stream play error
#[napi(string_enum)]
pub enum StreamPlayError {
    AlreadyPlaying,
    NotReady,
    SystemError,
}

/// Supported stream configuration
#[napi(object)]
pub struct SupportedStreamConfig {
    pub sample_rate: SampleRate,
    pub channel_count: ChannelCount,
    pub sample_width: u8,
}

/// Stream output configuration
#[napi(object)]
pub struct StreamOutputConfig {
    pub sample_rate: Option<SampleRate>,
    pub channels: Option<ChannelCount>,
    pub buffer_size: Option<u32>,
}

/// Global debug flag (defaults to false)
pub static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Simple debug macro
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        if $crate::types::DEBUG_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("[miniaudio-node] Debug: {}", format!($($arg)*));
        }
    };
}

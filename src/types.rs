use napi_derive::napi;
use std::sync::atomic::AtomicBool;

// ==================== TYPE ALIASES ====================

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

// ==================== ENUMS ====================

/// Error types for device operations
#[napi]
pub enum DevicesError {
    NoDevicesFound = 0,
    PermissionDenied = 1,
    InvalidDevice = 2,
    NotInitialized = 3,
}

/// Audio source function types (for generator sources)
#[napi]
pub enum SourceFunction {
    Sine = 0,
    Square = 1,
    Sawtooth = 2,
    Triangle = 3,
    WhiteNoise = 4,
    PinkNoise = 5,
    BrownNoise = 6,
}

/// Error types for stream operations
#[napi]
pub enum StreamError {
    NotPlaying = 0,
    EndOfFile = 1,
    InvalidData = 2,
    UnsupportedFormat = 3,
}

/// Play error for stream operations
#[napi]
pub enum PlayError {
    AlreadyPlaying = 0,
    NotLoaded = 1,
    SystemError = 2,
}

/// Seek error types
#[napi]
pub enum SeekError {
    InvalidPosition = 0,
    NotSeekable = 1,
    OutOfBounds = 2,
}

/// Decoder error types (for audio decoding operations)
#[napi]
pub enum DecoderError {
    InvalidFormat = 0,
    CorruptedData = 1,
    UnsupportedCodec = 2,
    IoError = 3,
}

/// Stream play error
#[napi]
pub enum StreamPlayError {
    AlreadyPlaying = 0,
    NotReady = 1,
    SystemError = 2,
}

// ==================== STRUCTS ====================

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

// ==================== GLOBAL DEBUG ====================

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

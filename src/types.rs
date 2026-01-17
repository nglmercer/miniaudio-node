use napi_derive::napi;

/// Audio device information structure
#[napi(object)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Audio player state enumeration
#[napi]
#[derive(Debug, PartialEq)]
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
}

#[napi(object)]
pub struct AudioMetadata {
    pub duration: f64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

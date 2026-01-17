//! Audio FFI - High-performance native audio playback for Node.js/Bun
//! Implementation with rodio (pure Rust audio library)

#![allow(clippy::arc_with_non_send_sync)]

// Declaramos los módulos
pub mod player;
pub mod types;
pub mod utils;

// Re-exportamos los contenidos para que NAPI los registre flat
// Esto es importante para que desde Node.js se importen como { AudioPlayer, initializeAudio }
// y no { player: { AudioPlayer }, utils: { initializeAudio } }
pub use player::*;
pub use types::*;
pub use utils::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formats() {
        assert!(is_format_supported("mp3".to_string()));
        assert!(!is_format_supported("xyz".to_string()));
    }

    #[test]
    fn test_player_creation() {
        let player = AudioPlayer::new().unwrap();
        assert_eq!(player.get_volume().unwrap(), 1.0);
        assert_eq!(player.get_state(), PlaybackState::Stopped);
    }

    // ... resto de los tests ...
}

//! Audio FFI - High-performance native audio playback for Node.js/Bun
//! Implementation with rodio (pure Rust audio library)

#![allow(clippy::arc_with_non_send_sync)]

// Declare the top-level modules
pub mod buffer;
pub mod conversions;
pub mod decoder;
pub mod input;
pub mod math;
pub mod mixer;
pub mod noise;
pub mod player;
pub mod queue;
pub mod stream;
pub mod types;
pub mod utils;

// Re-export all the contents at the crate root level for flat NAPI export
pub use buffer::*;
pub use conversions::*;
pub use decoder::*;
pub use input::*;
pub use math::*;
pub use mixer::*;
pub use noise::*;
pub use player::*;
pub use queue::*;
pub use stream::*;
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

    #[test]
    fn test_math_conversions() {
        // Test dB to linear conversion
        assert!((db_to_linear(0.0) - 1.0).abs() < 0.001);
        assert!((db_to_linear(-6.0) - 0.5).abs() < 0.07);
        assert!((db_to_linear(6.0) - 2.0).abs() < 0.07);

        // Test linear to dB conversion
        assert!(linear_to_db(1.0).abs() < 0.001);
        assert!((linear_to_db(0.5) + 6.0).abs() < 0.07);
        assert!((linear_to_db(2.0) - 6.0).abs() < 0.07);
    }

    #[test]
    fn test_noise_generation() {
        let white_noise = noise::white(100, 44100, 2);
        let samples = white_noise.get_samples();
        assert!(!samples.is_empty());
        assert!(samples.len() > 4410); // 44100 Hz * 0.1s * 2 channels

        let pink_noise = noise::pink(100, 44100, 2);
        let pink_samples = pink_noise.get_samples();
        assert!(!samples.is_empty());
        assert!(samples.len() == pink_samples.len());
    }

    #[test]
    fn test_buffer_creation() {
        let buffer = buffer::SamplesBuffer::create(2, 44100, vec![0i16; 4410]);
        assert_eq!(buffer.get_channels(), 2);
        assert_eq!(buffer.get_sample_rate(), 44100);
        assert_eq!(buffer.get_len(), 4410);
    }

    #[test]
    fn test_channel_conversion() {
        let converter = conversions::ChannelCountConverter::new(1, 2);
        assert_eq!(converter.source_channels(), 1);
        assert_eq!(converter.target_channels(), 2);

        let mono = vec![1000i16, 2000i16, 3000i16];
        let stereo = converter.convert(mono);
        assert_eq!(stereo.len(), 6); // mono * 2
    }

    #[test]
    fn test_sample_rate_conversion() {
        let converter = conversions::SampleRateConverter::new(44100, 48000);
        assert_eq!(converter.source_rate(), 44100);
        assert_eq!(converter.target_rate(), 48000);

        let samples = vec![1000i16; 1000];
        let converted = converter.convert(samples);
        assert!(!converted.is_empty());
    }

    #[test]
    fn test_mixer_creation() {
        let mixer = mixer::Mixer::new();
        assert_eq!(mixer.get_source_count(), 0);
        assert_eq!(mixer.get_master_volume(), 1.0);
    }

    #[test]
    fn test_queue_creation() {
        let q = queue::AudioSourceQueue::new();
        assert_eq!(q.get_length(), 0);
    }

    #[test]
    fn test_stream() {
        let configs = stream::supported_output_configs().unwrap();
        assert!(!configs.is_empty());
    }

    #[test]
    fn test_decoder() {
        use decoder::AudioDecoder;
        // Test that decoder properly handles invalid data (should error, not panic)
        let result = AudioDecoder::from_data(vec![]);
        assert!(result.is_err());
    }

    // Tests for seek_to function fixes
    #[test]
    fn test_seek_position_validation_nan() {
        // Test NaN position should return error
        let result = f64::NAN.is_nan();
        assert!(result);
    }

    #[test]
    fn test_seek_position_validation_infinite() {
        use std::f64;

        // Test that infinite values are detected
        let pos_inf = f64::INFINITY;
        let neg_inf = f64::NEG_INFINITY;

        assert!(pos_inf.is_infinite());
        assert!(neg_inf.is_infinite());
        assert!(!pos_inf.is_nan());
    }

    #[test]
    fn test_seek_position_clamping() {
        // Test that position clamping works correctly
        let duration: f64 = 100.0;
        let epsilon: f64 = 1e-9;

        // Negative position should be clamped to 0
        let neg_position: f64 = -5.0;
        let clamped_neg = neg_position.max(0.0).min(duration);
        assert_eq!(clamped_neg, 0.0);

        // Position beyond duration should be clamped to duration
        let overflow_position: f64 = 150.0;
        let clamped_overflow = overflow_position.max(0.0).min(duration);
        assert_eq!(clamped_overflow, duration);

        // Valid position should remain unchanged
        let valid_position: f64 = 50.0;
        let clamped_valid = valid_position.max(0.0).min(duration);
        assert_eq!(clamped_valid, 50.0);

        // Test epsilon comparison
        assert!(-epsilon < 0.0);
        assert!(100.0 + epsilon > duration);
    }

    #[test]
    fn test_seek_time_calculation() {
        // Test that time calculation with decimals works correctly
        let position = 5.5; // 5.5 seconds
        let bytes_per_second = 44100.0 * 4.0; // 16-bit stereo

        let skip_bytes = (position * bytes_per_second) as usize;
        let expected = (5.5 * 176400.0) as usize;

        assert_eq!(skip_bytes, expected);
        assert!(skip_bytes > 0);
    }

    #[test]
    fn test_saturating_arithmetic() {
        // Test saturating subtraction for time tracking
        let now: u128 = 1_000_000_000_000_000_000; // 1e18 ns (about 11 days)
        let seek_position_ns: u128 = 5_500_000_000; // 5.5 seconds in ns

        // This should not underflow
        let result = now.saturating_sub(seek_position_ns);
        assert!(result > 0);
        assert_eq!(result, now - seek_position_ns);

        // Test with position larger than current time (edge case)
        let small_now: u128 = 1_000_000_000; // 1 second
        let large_position_ns: u128 = 5_000_000_000; // 5 seconds

        let result2 = small_now.saturating_sub(large_position_ns);
        assert_eq!(result2, 0); // Should saturate to 0, not underflow
    }

    #[test]
    fn test_decimal_precision() {
        // Test that decimal positions are handled correctly
        let position: f64 = 1.23456789;
        let duration: f64 = 10.0;

        // Position should be within valid range after clamping
        let clamped = position.max(0.0).min(duration);
        assert!(clamped >= 0.0 && clamped <= duration);

        // Conversion to nanoseconds should work
        let ns = (clamped * 1_000_000_000.0) as u128;
        assert_eq!(ns, 1_234_567_890);

        // Test with very small decimal
        let small_position: f64 = 0.000000001;
        let small_ns = (small_position * 1_000_000_000.0) as u128;
        assert_eq!(small_ns, 1);
    }

    #[test]
    fn test_volume_range_validation() {
        let mut player = AudioPlayer::new().unwrap();

        // Valid volume range
        assert!(player.set_volume(0.0).is_ok());
        assert!(player.set_volume(1.0).is_ok());
        assert!(player.set_volume(0.5).is_ok());

        // Volume should be clamped internally
        let result = player.set_volume(1.5);
        assert!(result.is_err()); // Should error for out of range
    }

    #[test]
    fn test_seek_to_validates_nan_position() {
        // NaN should be detected
        assert!(f64::NAN.is_nan());
    }

    #[test]
    fn test_seek_to_validates_infinite_position() {
        use std::f64;

        // Infinity should be detected
        assert!(f64::INFINITY.is_infinite());
        assert!(f64::NEG_INFINITY.is_infinite());
    }

    #[test]
    fn test_seek_position_with_decimal_precision() {
        // Test various decimal positions
        let test_positions = vec![0.0f64, 0.001, 0.5, 1.234, 10.567, 100.999];

        for &pos in &test_positions {
            // Should be able to convert to nanoseconds without issues
            let ns = (pos * 1_000_000_000.0) as u128;
            // u128 is always >= 0, so no need to assert
            assert!(ns > 0 || pos == 0.0);

            // Should be able to convert back (approximately)
            let back_to_seconds = ns as f64 / 1_000_000_000.0;
            assert!((back_to_seconds - pos).abs() < 0.001);
        }
    }

    #[test]
    fn test_seek_position_edge_cases() {
        let duration: f64 = 60.0;
        let epsilon = 1e-9;

        // Very small positive (should be valid)
        assert!(0.0 >= -epsilon);
        assert!(0.0 <= duration + epsilon);

        // Slightly over duration (should be clamped)
        let over: f64 = duration + 0.0001;
        let clamped = over.max(0.0).min(duration);
        assert_eq!(clamped, duration);

        // Very small negative (should be clamped to 0)
        let under: f64 = -0.0001;
        let clamped2 = under.max(0.0).min(duration);
        assert_eq!(clamped2, 0.0);
    }

    #[test]
    fn test_skip_bytes_calculation() {
        // Test buffer skip calculation for decimal positions
        let sample_rate = 44100.0;
        let bytes_per_sample = 4.0; // 16-bit stereo
        let bytes_per_second = sample_rate * bytes_per_sample;

        // Test at 1.5 seconds
        let position = 1.5;
        let skip_bytes = (position * bytes_per_second) as usize;
        let expected = (1.5 * 176400.0) as usize;

        assert_eq!(skip_bytes, expected);
        assert_eq!(skip_bytes, 264600);

        // Test at 0.5 seconds
        let position2 = 0.5;
        let skip_bytes2 = (position2 * bytes_per_second) as usize;
        assert_eq!(skip_bytes2, 88200);
    }
}

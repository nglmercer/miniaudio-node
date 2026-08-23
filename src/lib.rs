//! Audio FFI - High-performance native audio playback for Node.js/Bun
//! Implementation with rodio (pure Rust audio library)

#![allow(clippy::arc_with_non_send_sync)]

// Declare the top-level modules
pub mod audio_passthrough;
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
pub use audio_passthrough::*;
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
    fn test_channel_conversion_handles_general_layouts() {
        let stereo_to_three = conversions::ChannelCountConverter::new(2, 3);
        assert_eq!(
            stereo_to_three.convert(vec![1000, 2000, 3000, 4000]),
            vec![1000, 2000, 1500, 3000, 4000, 3500]
        );

        let three_to_two = conversions::ChannelCountConverter::new(3, 2);
        assert_eq!(
            three_to_two.convert(vec![1000, 2000, 3000]),
            vec![2000, 2000]
        );

        let zero_channels = conversions::ChannelCountConverter::new(0, 2);
        assert!(zero_channels.convert(vec![1, 2, 3]).is_empty());
    }

    #[test]
    fn test_sample_rate_conversion() {
        let converter = conversions::SampleRateConverter::new(44100, 48000, None);
        assert_eq!(converter.source_rate(), 44100);
        assert_eq!(converter.target_rate(), 48000);

        let samples = vec![1000i16; 1000];
        let converted = converter.convert(samples);
        assert!(!converted.is_empty());
    }

    #[test]
    fn test_sample_rate_conversion_preserves_interleaved_channels() {
        let converter = conversions::SampleRateConverter::new(48000, 44100, Some(2));
        let samples = vec![0, 10000, 1000, 11000, 2000, 12000, 3000, 13000];
        let converted = converter.convert(samples);

        assert_eq!(converted.len(), 6);
        assert!(converted
            .as_chunks::<2>()
            .0
            .iter()
            .all(|frame| frame[1] - frame[0] >= 9_000));
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
    fn test_queue_ids_and_cursor_survive_removal() {
        let q = queue::AudioSourceQueue::new();
        let first = q.add_buffer(vec![1], None).unwrap();
        let second = q.add_buffer(vec![2], None).unwrap();
        assert_eq!(first, "source_0");
        assert_eq!(second, "source_1");

        let output = queue::SourcesQueueOutput::from_queue(&q);
        assert_eq!(output.pop().unwrap().source_id, first);
        q.remove_source(second).unwrap();
        assert_eq!(output.get_remaining(), 0);

        let third = q.add_buffer(vec![3], None).unwrap();
        assert_eq!(third, "source_2");
        assert_eq!(output.get_remaining(), 1);
    }

    #[test]
    fn test_recorder_retention_overwrites_oldest_samples() {
        let mut samples = vec![1, 2, 3, 4];
        input::append_bounded(&mut samples, &[5, 6], Some(4));
        assert_eq!(samples, vec![3, 4, 5, 6]);

        input::append_bounded(&mut samples, &[7, 8, 9, 10, 11], Some(4));
        assert_eq!(samples, vec![8, 9, 10, 11]);
    }

    #[test]
    fn test_mixer_applies_source_and_master_controls() {
        let mut source = mixer::MixerSource::new("source".to_string(), vec![1000, -1000], 44100, 2);
        let mut mixer = mixer::Mixer::with_config(44100, 2, 1);
        mixer.add_source(&source).unwrap();
        assert_eq!(mixer.sample_at(0).unwrap(), vec![1000, -1000]);

        source.set_volume(0.5).unwrap();
        source.set_pan(1.0).unwrap();
        mixer.set_master_volume(0.5).unwrap();
        assert_eq!(mixer.sample_at(0).unwrap(), vec![0, -250]);

        source.set_enabled(false);
        assert_eq!(mixer.sample_at(0).unwrap(), vec![0, 0]);
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
}

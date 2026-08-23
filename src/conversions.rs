//! Audio format conversion utilities

use napi::{Error, Result, Status};
use napi_derive::napi;

/// Convert interleaved samples between channel layouts.
///
/// Existing channels are preserved where possible. When reducing the channel
/// count, source channels are grouped into the target channels and averaged;
/// when expanding it, additional channels receive the average of the source
/// frame. This keeps every output frame at exactly `target_channels` samples.
pub(crate) fn convert_channels_f32(
    samples: &[f32],
    source_channels: u16,
    target_channels: u16,
) -> Vec<f32> {
    let src = source_channels as usize;
    let dst = target_channels as usize;
    if src == 0 || dst == 0 {
        return Vec::new();
    }

    let frame_count = samples.len() / src;
    let samples = &samples[..frame_count * src];
    if src == dst {
        return samples.to_vec();
    }

    let mut output = Vec::with_capacity(frame_count * dst);
    for frame in samples.chunks_exact(src) {
        let source_average = frame.iter().copied().sum::<f32>() / src as f32;

        if dst == 1 {
            output.push(source_average);
            continue;
        }

        if src == 1 {
            output.extend(std::iter::repeat_n(frame[0], dst));
            continue;
        }

        if dst < src {
            for target_index in 0..dst {
                let mut sum = 0.0;
                let mut count = 0;
                for source_index in (target_index..src).step_by(dst) {
                    sum += frame[source_index];
                    count += 1;
                }
                output.push(if count == 0 {
                    source_average
                } else {
                    sum / count as f32
                });
            }
        } else {
            output.extend_from_slice(frame);
            output.extend(std::iter::repeat_n(source_average, dst - src));
        }
    }

    output
}

/// Resample interleaved samples using a fractional source position for every
/// destination frame. Interpolation is performed independently per channel.
pub(crate) fn resample_f32(
    samples: &[f32],
    source_rate: u32,
    target_rate: u32,
    channels: u16,
) -> Vec<f32> {
    let channel_count = channels as usize;
    if source_rate == 0 || target_rate == 0 || channel_count == 0 {
        return Vec::new();
    }

    let source_frames = samples.len() / channel_count;
    let samples = &samples[..source_frames * channel_count];
    if source_frames == 0 {
        return Vec::new();
    }
    if source_rate == target_rate {
        return samples.to_vec();
    }

    let ratio = target_rate as f64 / source_rate as f64;
    let target_frames = (source_frames as f64 * ratio).floor() as usize;
    let source_step = source_rate as f64 / target_rate as f64;
    let mut output = Vec::with_capacity(target_frames * channel_count);

    for target_frame in 0..target_frames {
        let source_position = target_frame as f64 * source_step;
        let source_index = source_position.floor() as usize;
        let fraction = (source_position - source_index as f64) as f32;
        let next_index = (source_index + 1).min(source_frames - 1);

        for channel in 0..channel_count {
            let first = samples[source_index * channel_count + channel];
            let second = samples[next_index * channel_count + channel];
            output.push(first + (second - first) * fraction);
        }
    }

    output
}

fn clamp_i16(value: f32) -> i16 {
    value.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

/// Parameters for channel count conversion
#[napi(object)]
pub struct ChannelCountConversion {
    pub source_channels: u16,
    pub target_channels: u16,
}

/// Parameters for sample rate conversion
#[napi(object)]
pub struct SampleRateConversion {
    pub source_rate: u32,
    pub target_rate: u32,
}

/// Parameters for sample type conversion
#[napi(object)]
pub struct SampleTypeConversion {
    pub source_bits: u8,
    pub target_bits: u8,
}

/// Channel count converter - handles converting between mono, stereo, and multi-channel audio
#[napi]
pub struct ChannelCountConverter {
    source_channels: u16,
    target_channels: u16,
}

#[napi]
impl ChannelCountConverter {
    #[napi(constructor)]
    pub fn new(source_channels: u16, target_channels: u16) -> Result<Self> {
        if source_channels == 0 {
            return Err(Error::new(
                Status::InvalidArg,
                "Source channel count must be greater than zero",
            ));
        }
        if target_channels == 0 {
            return Err(Error::new(
                Status::InvalidArg,
                "Target channel count must be greater than zero",
            ));
        }

        Ok(Self {
            source_channels,
            target_channels,
        })
    }

    /// Convert audio samples from source channel count to target channel count
    #[napi]
    pub fn convert(&self, samples: Vec<i16>) -> Vec<i16> {
        let src = self.source_channels as usize;
        let dst = self.target_channels as usize;

        if src == 0 || dst == 0 {
            return Vec::new();
        }

        let frame_count = samples.len() / src;
        let samples = &samples[..frame_count * src];
        if src == dst {
            return samples.to_vec();
        }

        let mut output = Vec::with_capacity(frame_count * dst);
        for frame in samples.chunks_exact(src) {
            let sum: i64 = frame.iter().map(|&sample| sample as i64).sum();
            let source_average = (sum / src as i64).clamp(i16::MIN as i64, i16::MAX as i64) as i16;

            if dst == 1 {
                output.push(source_average);
            } else if src == 1 {
                output.extend(std::iter::repeat_n(frame[0], dst));
            } else if dst < src {
                for target_index in 0..dst {
                    let mut group_sum = 0i64;
                    let mut group_count = 0i64;
                    for source_index in (target_index..src).step_by(dst) {
                        group_sum += frame[source_index] as i64;
                        group_count += 1;
                    }
                    output.push(
                        (group_sum / group_count.max(1)).clamp(i16::MIN as i64, i16::MAX as i64)
                            as i16,
                    );
                }
            } else {
                output.extend_from_slice(frame);
                output.extend(std::iter::repeat_n(source_average, dst - src));
            }
        }

        output
    }

    #[napi]
    pub fn source_channels(&self) -> u16 {
        self.source_channels
    }

    #[napi]
    pub fn target_channels(&self) -> u16 {
        self.target_channels
    }
}

/// Sample rate converter - handles converting between different sample rates (e.g., 44100 to 48000)
#[napi]
pub struct SampleRateConverter {
    source_rate: u32,
    target_rate: u32,
    channels: u16,
}

#[napi]
impl SampleRateConverter {
    #[napi(constructor)]
    pub fn new(source_rate: u32, target_rate: u32, channels: Option<u16>) -> Result<Self> {
        if source_rate == 0 {
            return Err(Error::new(
                Status::InvalidArg,
                "Source sample rate must be greater than zero",
            ));
        }
        if target_rate == 0 {
            return Err(Error::new(
                Status::InvalidArg,
                "Target sample rate must be greater than zero",
            ));
        }
        let channels = channels.unwrap_or(1);
        if channels == 0 {
            return Err(Error::new(
                Status::InvalidArg,
                "Channel count must be greater than zero",
            ));
        }

        Ok(Self {
            source_rate,
            target_rate,
            channels,
        })
    }

    /// Convert audio samples from source rate to target rate using linear interpolation
    #[napi]
    pub fn convert(&self, samples: Vec<i16>) -> Vec<i16> {
        resample_f32(
            &samples
                .iter()
                .map(|&sample| sample as f32)
                .collect::<Vec<_>>(),
            self.source_rate,
            self.target_rate,
            self.channels,
        )
        .into_iter()
        .map(clamp_i16)
        .collect()
    }

    #[napi]
    pub fn source_rate(&self) -> u32 {
        self.source_rate
    }

    #[napi]
    pub fn target_rate(&self) -> u32 {
        self.target_rate
    }

    #[napi]
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

/// Sample type converter - handles converting between different bit depths (8, 16, 24, 32 bit)
#[napi]
pub struct SampleTypeConverter {
    source_bits: u8,
    target_bits: u8,
}

#[napi]
impl SampleTypeConverter {
    #[napi(constructor)]
    pub fn new(source_bits: u8, target_bits: u8) -> Result<Self> {
        if !matches!(source_bits, 8 | 16 | 24 | 32) || !matches!(target_bits, 8 | 16 | 24 | 32) {
            return Err(Error::new(
                Status::InvalidArg,
                "Sample bit depths must be one of 8, 16, 24, or 32",
            ));
        }

        Ok(Self {
            source_bits,
            target_bits,
        })
    }

    /// Convert between different sample bit depths
    #[napi]
    pub fn convert(&self, samples: Vec<i32>) -> Result<Vec<i32>> {
        let converted = match (self.source_bits, self.target_bits) {
            // Identity conversion is explicit and supported for all valid
            // representations.
            (source, target) if source == target => samples,

            // 24-bit to 16-bit (truncate and clamp)
            (24, 16) => samples
                .iter()
                .map(|&s| s.clamp(-8388608, 8388607) >> 8)
                .collect(),

            // 32-bit (float) to 16-bit (clamp to i16 range)
            (32, 16) => samples
                .iter()
                .map(|&s| {
                    let f = s as f32 / 2147483648.0;
                    (f.clamp(-1.0, 1.0) * 32767.0) as i32
                })
                .collect(),

            // 8-bit unsigned to 16-bit signed
            (8, 16) => samples.iter().map(|&s| (s - 128) * 256).collect(),

            // 16-bit to 8-bit (truncate)
            (16, 8) => samples
                .iter()
                .map(|&s| ((s / 256 + 128) as u8) as i32)
                .collect(),

            // 16-bit to 24-bit (pad)
            (16, 24) => samples.iter().map(|&s| s << 8).collect(),

            // 16-bit to 32-bit (integer)
            (16, 32) => samples.iter().map(|&s| s << 16).collect(),

            // 24-bit to 32-bit (integer)
            (24, 32) => samples.iter().map(|&s| s << 8).collect(),

            // 32-bit to 24-bit (truncate)
            (32, 24) => samples.iter().map(|&s| s >> 8).collect(),

            // 8-bit to 24-bit (pad and shift)
            (8, 24) => samples.iter().map(|&s| (s - 128) << 16).collect(),

            // 8-bit to 32-bit (pad and shift)
            (8, 32) => samples.iter().map(|&s| (s - 128) << 24).collect(),

            // 24-bit to 8-bit (truncate)
            (24, 8) => samples.iter().map(|&s| (s >> 16) + 128).collect(),

            // 32-bit to 8-bit (truncate)
            (32, 8) => samples
                .iter()
                .map(|&s| {
                    let clamped = if s < -2147483648 { -2147483648 } else { s };
                    let reduced = clamped >> 24;
                    reduced + 128
                })
                .collect(),

            _ => {
                return Err(Error::new(
                    Status::InvalidArg,
                    format!(
                        "Unsupported sample conversion: {}-bit to {}-bit",
                        self.source_bits, self.target_bits
                    ),
                ))
            }
        };
        Ok(converted)
    }

    #[napi]
    pub fn source_bits(&self) -> u8 {
        self.source_bits
    }

    #[napi]
    pub fn target_bits(&self) -> u8 {
        self.target_bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_type_converter_validates_bit_depths_and_supports_identity() {
        assert!(SampleTypeConverter::new(12, 16).is_err());
        assert!(SampleTypeConverter::new(16, 20).is_err());

        let converter = SampleTypeConverter::new(16, 16).unwrap();
        assert_eq!(converter.convert(vec![-123, 456]).unwrap(), vec![-123, 456]);
    }
}

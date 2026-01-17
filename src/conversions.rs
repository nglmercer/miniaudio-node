//! Audio format conversion utilities

use napi_derive::napi;

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
    pub fn new(source_channels: u16, target_channels: u16) -> Self {
        Self {
            source_channels,
            target_channels,
        }
    }

    /// Convert audio samples from source channel count to target channel count
    #[napi]
    pub fn convert(&self, samples: Vec<i16>) -> Vec<i16> {
        let src = self.source_channels as usize;
        let dst = self.target_channels as usize;

        if src == dst {
            return samples;
        }

        let output_len = (samples.len() / src) * dst;
        let mut output = Vec::with_capacity(output_len);

        if src == 1 && dst == 2 {
            // Mono to stereo (duplicate samples)
            for sample in samples {
                output.push(sample);
                output.push(sample);
            }
        } else if src == 2 && dst == 1 {
            // Stereo to mono (average channels)
            for chunk in samples.chunks(2) {
                if chunk.len() == 2 {
                    let avg = ((chunk[0] as i32 + chunk[1] as i32) / 2) as i16;
                    output.push(avg);
                }
            }
        } else {
            // For other conversions, upsample by duplicating or downsample by averaging
            if dst > src {
                // Upsample: duplicate samples
                for sample in samples {
                    for _ in 0..dst {
                        output.push(sample / src as i16);
                    }
                }
            } else {
                // Downsample: average samples in groups
                for chunk in samples.chunks(src) {
                    if chunk.len() == src {
                        let sum: i32 = chunk.iter().map(|&s| s as i32).sum();
                        output.push((sum / dst as i32) as i16);
                    }
                }
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
}

#[napi]
impl SampleRateConverter {
    #[napi(constructor)]
    pub fn new(source_rate: u32, target_rate: u32) -> Self {
        Self {
            source_rate,
            target_rate,
        }
    }

    /// Convert audio samples from source rate to target rate using linear interpolation
    #[napi]
    pub fn convert(&self, samples: Vec<i16>) -> Vec<i16> {
        if self.source_rate == self.target_rate {
            return samples;
        }

        let ratio = self.target_rate as f64 / self.source_rate as f64;
        let output_len = (samples.len() as f64 * ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        if ratio > 1.0 {
            // Upsample - use linear interpolation
            for i in 0..output_len {
                let pos = i as f64 / ratio;
                let idx = pos as usize;
                let frac = pos - idx as f64;

                if idx + 1 < samples.len() {
                    let s1 = samples[idx] as f64;
                    let s2 = samples[idx + 1] as f64;
                    let interpolated = s1 + (s2 - s1) * frac;
                    output.push(interpolated as i16);
                } else if idx < samples.len() {
                    output.push(samples[idx]);
                }
            }
        } else {
            // Downsample - take every nth sample
            let stride = (1.0 / ratio) as usize;
            for i in 0..output_len {
                let idx = i * stride;
                if idx < samples.len() {
                    output.push(samples[idx]);
                }
            }
        }

        output
    }

    #[napi]
    pub fn source_rate(&self) -> u32 {
        self.source_rate
    }

    #[napi]
    pub fn target_rate(&self) -> u32 {
        self.target_rate
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
    pub fn new(source_bits: u8, target_bits: u8) -> Self {
        Self {
            source_bits,
            target_bits,
        }
    }

    /// Convert between different sample bit depths
    #[napi]
    pub fn convert(&self, samples: Vec<i32>) -> Vec<i32> {
        match (self.source_bits, self.target_bits) {
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

            // Return samples as-is for other conversions
            _ => samples,
        }
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

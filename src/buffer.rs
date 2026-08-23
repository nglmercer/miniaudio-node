//! Audio buffer types for sample data storage and manipulation

use napi::{Error, Result, Status};
use napi_derive::napi;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A buffer containing audio samples
#[napi]
pub struct SamplesBuffer {
    samples: Arc<Mutex<Vec<i16>>>,
    channels: u16,
    sample_rate: u32,
}

impl SamplesBuffer {
    fn new(channels: u16, sample_rate: u32, samples: Vec<i16>) -> Self {
        SamplesBuffer {
            sample_rate,
            channels,
            samples: Arc::new(Mutex::new(samples)),
        }
    }

    fn validate_format(channels: u32, sample_rate: u32, sample_count: usize) -> Result<u16> {
        if channels == 0 {
            return Err(Error::new(
                Status::InvalidArg,
                "Channel count must be greater than zero",
            ));
        }
        if channels > u16::MAX as u32 {
            return Err(Error::new(
                Status::InvalidArg,
                format!("Channel count must not exceed {}", u16::MAX),
            ));
        }
        if sample_rate == 0 {
            return Err(Error::new(
                Status::InvalidArg,
                "Sample rate must be greater than zero",
            ));
        }
        if !sample_count.is_multiple_of(channels as usize) {
            return Err(Error::new(
                Status::InvalidArg,
                "Interleaved samples must contain a complete frame",
            ));
        }
        Ok(channels as u16)
    }
}

#[napi]
impl SamplesBuffer {
    /// Create a new samples buffer
    #[napi(constructor)]
    pub fn create(channels: u32, sample_rate: u32, samples: Vec<i16>) -> Result<Self> {
        let channels = Self::validate_format(channels, sample_rate, samples.len())?;
        Ok(Self::new(channels, sample_rate, samples))
    }

    /// Get the number of channels in this buffer (1=mono, 2=stereo)
    #[napi]
    pub fn get_channels(&self) -> u32 {
        self.channels as u32
    }

    /// Get the sample rate of this buffer
    #[napi]
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the number of samples in this buffer
    #[napi]
    pub fn get_len(&self) -> u32 {
        let samples = self.samples.lock().unwrap_or_else(|e| e.into_inner());
        samples.len() as u32
    }

    /// Get the duration of this buffer in seconds
    #[napi]
    pub fn get_duration(&self) -> f64 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0.0;
        }
        self.get_len() as f64 / (self.sample_rate as f64 * self.channels as f64)
    }

    /// Get a copy of the samples in this buffer
    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let samples = self.samples.lock().unwrap_or_else(|e| e.into_inner());
        samples.clone()
    }

    /// Create a buffer from raw bytes (16-bit little-endian samples)
    #[napi(factory)]
    pub fn from_bytes(bytes: Vec<u8>, channels: u32, sample_rate: u32) -> Result<Self> {
        if !bytes.len().is_multiple_of(2) {
            return Err(Error::new(
                Status::InvalidArg,
                "Raw PCM bytes must contain complete 16-bit samples",
            ));
        }
        let samples: Vec<i16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|chunk| i16::from_le_bytes(*chunk))
            .collect();
        let channels = Self::validate_format(channels, sample_rate, samples.len())?;
        Ok(Self::new(channels, sample_rate, samples))
    }

    /// Play this buffer asynchronously; returns after the output stream starts.
    #[napi]
    pub fn play(&self) -> napi::Result<()> {
        use rodio::{OutputStreamBuilder, Sink, Source};

        use std::sync::mpsc;

        let samples_i16 = self
            .samples
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        // Convert i16 samples to f32 for rodio
        let samples_f32: Vec<f32> = samples_i16
            .into_iter()
            .map(|s| s as f32 / 32768.0)
            .collect();

        // Create a source from the f32 samples
        #[derive(Clone)]
        struct VecSource {
            samples: Vec<f32>,
            index: usize,
            sample_rate: u32,
            channels: u16,
        }

        impl Iterator for VecSource {
            type Item = f32;

            fn next(&mut self) -> Option<Self::Item> {
                if self.index < self.samples.len() {
                    let sample = self.samples[self.index];
                    self.index += 1;
                    Some(sample)
                } else {
                    None
                }
            }
        }

        impl Source for VecSource {
            fn current_span_len(&self) -> Option<usize> {
                Some(self.samples.len() - self.index)
            }

            fn channels(&self) -> u16 {
                self.channels
            }

            fn sample_rate(&self) -> u32 {
                self.sample_rate
            }

            fn total_duration(&self) -> Option<Duration> {
                Some(Duration::from_secs_f64(
                    (self.samples.len() as f64) / (self.sample_rate as f64 * self.channels as f64),
                ))
            }
        }

        let (ready_tx, ready_rx) = mpsc::channel();
        let sample_rate = self.sample_rate;
        let channels = self.channels;

        // CoreAudio's output stream owns a non-Send property-listener callback.
        // Create and retain the stream entirely inside the worker instead of
        // moving it across the thread boundary.
        std::thread::spawn(move || {
            let stream = match OutputStreamBuilder::open_default_stream() {
                Ok(stream) => stream,
                Err(error) => {
                    let _ = ready_tx.send(Err(error.to_string()));
                    return;
                }
            };
            let sink = Sink::connect_new(stream.mixer());
            let source = VecSource {
                samples: samples_f32,
                index: 0,
                sample_rate,
                channels,
            };
            sink.append(source);
            sink.play();
            let _ = ready_tx.send(Ok(()));
            sink.sleep_until_end();
            drop(stream);
        });

        ready_rx
            .recv()
            .map_err(|_| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    "Audio playback worker exited before starting",
                )
            })?
            .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error))
    }
}

/// Static buffer that owns its audio data
#[napi]
pub struct StaticSamplesBuffer {
    inner: SamplesBuffer,
}

#[napi]
impl StaticSamplesBuffer {
    #[napi(constructor)]
    pub fn new(channels: u32, sample_rate: u32, samples: Vec<i16>) -> Result<Self> {
        let channels = SamplesBuffer::validate_format(channels, sample_rate, samples.len())?;
        Ok(StaticSamplesBuffer {
            inner: SamplesBuffer::new(channels, sample_rate, samples),
        })
    }

    #[napi]
    pub fn get_inner(&self) -> SamplesBuffer {
        SamplesBuffer {
            sample_rate: self.inner.sample_rate,
            channels: self.inner.channels,
            samples: self.inner.samples.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_reject_invalid_audio_metadata_and_alignment() {
        assert!(SamplesBuffer::create(0, 44_100, Vec::new()).is_err());
        assert!(SamplesBuffer::create(2, 0, Vec::new()).is_err());
        assert!(SamplesBuffer::create(70_000, 44_100, Vec::new()).is_err());
        assert!(SamplesBuffer::create(2, 44_100, vec![1]).is_err());
        assert!(SamplesBuffer::from_bytes(vec![1], 1, 44_100).is_err());

        let buffer = SamplesBuffer::from_bytes(vec![0x34, 0x12], 1, 44_100).unwrap();
        assert_eq!(buffer.get_samples(), vec![0x1234]);
        assert!((buffer.get_duration() - (1.0 / 44_100.0)).abs() < f64::EPSILON);
    }
}

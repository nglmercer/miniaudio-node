//! Audio buffer types for sample data storage and manipulation

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
    pub fn new(channels: u16, sample_rate: u32, samples: Vec<i16>) -> Self {
        SamplesBuffer {
            sample_rate,
            channels,
            samples: Arc::new(Mutex::new(samples)),
        }
    }
}

#[napi]
impl SamplesBuffer {
    /// Create a new samples buffer
    #[napi(constructor)]
    pub fn create(channels: u32, sample_rate: u32, samples: Vec<i16>) -> Self {
        Self::new(channels as u16, sample_rate, samples)
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
        let samples = self.samples.lock().unwrap();
        samples.len() as u32
    }

    /// Get the duration of this buffer in seconds
    #[napi]
    pub fn get_duration(&self) -> f64 {
        self.get_len() as f64 / (self.sample_rate as f64 * self.channels as f64)
    }

    /// Get a copy of the samples in this buffer
    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let samples = self.samples.lock().unwrap();
        samples.clone()
    }

    /// Create a buffer from raw bytes (16-bit little-endian samples)
    #[napi(factory)]
    pub fn from_bytes(bytes: Vec<u8>, channels: u32, sample_rate: u32) -> Self {
        let samples: Vec<i16> = bytes
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        Self::new(channels as u16, sample_rate, samples)
    }

    /// Play this buffer with the given sink
    #[napi]
    pub fn play(&self) -> napi::Result<()> {
        use rodio::{OutputStreamBuilder, Sink, Source};

        let stream = OutputStreamBuilder::open_default_stream()
            .map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))?;

        let sink = Sink::connect_new(stream.mixer());
        let samples_i16 = self.samples.lock().unwrap().clone();

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

        let source = VecSource {
            samples: samples_f32,
            index: 0,
            sample_rate: self.sample_rate,
            channels: self.channels,
        };

        sink.append(source);
        sink.play();

        // Block until playback finishes to keep stream alive
        sink.sleep_until_end();

        Ok(())
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
    pub fn new(channels: u32, sample_rate: u32, samples: Vec<i16>) -> Self {
        StaticSamplesBuffer {
            inner: SamplesBuffer::new(channels as u16, sample_rate, samples),
        }
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

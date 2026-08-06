use napi_derive::napi;
use rand::Rng;
use std::sync::{Arc, Mutex};

/// Simple white noise source
#[napi]
pub struct White {
    samples: Arc<Mutex<Vec<i16>>>,
}

#[napi]
impl White {
    #[napi(constructor)]
    pub fn new(duration_ms: u32, sample_rate: u32, channels: u16) -> Self {
        let total_samples =
            ((duration_ms as f64 * sample_rate as f64 / 1000.0) as usize) * channels as usize;
        let mut samples = Vec::with_capacity(total_samples);
        let mut rng = rand::rng();

        for _ in 0..total_samples {
            samples.push(rng.random());
        }

        Self {
            samples: Arc::new(Mutex::new(samples)),
        }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        self.samples.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[napi]
pub struct WhiteGenerator {
    sample_rate: u32,
    channels: u16,
}

#[napi]
impl WhiteGenerator {
    #[napi(constructor)]
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }

    #[napi]
    pub fn generate(&self, duration_ms: u32) -> Vec<i16> {
        let total_samples = ((duration_ms as f64 * self.sample_rate as f64 / 1000.0) as usize)
            * self.channels as usize;
        let mut samples = Vec::with_capacity(total_samples);
        let mut rng = rand::rng();

        for _ in 0..total_samples {
            samples.push(rng.random());
        }

        samples
    }
}

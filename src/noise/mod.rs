//! Audio noise generation - various types of noise for synthesis and testing

use napi_derive::napi;
use std::sync::{Arc, Mutex};

/// Blue noise generator - high frequency emphasis
#[napi]
pub struct BlueNoise {
    samples: Arc<Mutex<Vec<i16>>>,
    position: Arc<Mutex<usize>>,
}

#[napi]
impl BlueNoise {
    #[napi(constructor)]
    pub fn new(duration_ms: u32, sample_rate: u32, channels: u16) -> Self {
        let samples = generate_blue_noise(duration_ms, sample_rate, channels);
        Self {
            samples: Arc::new(Mutex::new(samples)),
            position: Arc::new(Mutex::new(0)),
        }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let samples = self.samples.lock().unwrap();
        samples.clone()
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        let mut pos = self.position.lock().unwrap();
        let samples = self.samples.lock().unwrap();

        if *pos >= samples.len() {
            return None;
        }
        let sample = samples[*pos];
        *pos += 1;
        Some(sample)
    }

    #[napi]
    pub fn reset(&self) {
        *self.position.lock().unwrap() = 0;
    }
}

/// Brownian noise (random walk) - low frequency emphasis
#[napi]
pub struct BrownianNoise {
    samples: Arc<Mutex<Vec<i16>>>,
    position: Arc<Mutex<usize>>,
}

#[napi]
impl BrownianNoise {
    #[napi(constructor)]
    pub fn new(duration_ms: u32, sample_rate: u32, channels: u16) -> Self {
        let samples = generate_brownian_noise(duration_ms, sample_rate, channels);
        Self {
            samples: Arc::new(Mutex::new(samples)),
            position: Arc::new(Mutex::new(0)),
        }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let samples = self.samples.lock().unwrap();
        samples.clone()
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        let mut pos = self.position.lock().unwrap();
        let samples = self.samples.lock().unwrap();

        if *pos >= samples.len() {
            return None;
        }
        let sample = samples[*pos];
        *pos += 1;
        Some(sample)
    }

    #[napi]
    pub fn reset(&self) {
        *self.position.lock().unwrap() = 0;
    }
}

/// Pink noise generator - equal power per octave
#[napi]
pub struct PinkNoise {
    samples: Arc<Mutex<Vec<i16>>>,
    position: Arc<Mutex<usize>>,
}

#[napi]
impl PinkNoise {
    #[napi(constructor)]
    pub fn new(duration_ms: u32, sample_rate: u32, channels: u16) -> Self {
        let samples = generate_pink_noise(duration_ms, sample_rate, channels);
        Self {
            samples: Arc::new(Mutex::new(samples)),
            position: Arc::new(Mutex::new(0)),
        }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let samples = self.samples.lock().unwrap();
        samples.clone()
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        let mut pos = self.position.lock().unwrap();
        let samples = self.samples.lock().unwrap();

        if *pos >= samples.len() {
            return None;
        }
        let sample = samples[*pos];
        *pos += 1;
        Some(sample)
    }

    #[napi]
    pub fn reset(&self) {
        *self.position.lock().unwrap() = 0;
    }
}

/// Velvet noise - sparse, crackly noise
#[napi]
pub struct VelvetNoise {
    samples: Arc<Mutex<Vec<i16>>>,
    position: Arc<Mutex<usize>>,
}

#[napi]
impl VelvetNoise {
    #[napi(constructor)]
    pub fn new(duration_ms: u32, sample_rate: u32, channels: u16) -> Self {
        let samples = generate_velvet_noise(duration_ms, sample_rate, channels);
        Self {
            samples: Arc::new(Mutex::new(samples)),
            position: Arc::new(Mutex::new(0)),
        }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let samples = self.samples.lock().unwrap();
        samples.clone()
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        let mut pos = self.position.lock().unwrap();
        let samples = self.samples.lock().unwrap();

        if *pos >= samples.len() {
            return None;
        }
        let sample = samples[*pos];
        *pos += 1;
        Some(sample)
    }

    #[napi]
    pub fn reset(&self) {
        *self.position.lock().unwrap() = 0;
    }
}

/// Violet noise - very high frequency emphasis
#[napi]
pub struct VioletNoise {
    samples: Arc<Mutex<Vec<i16>>>,
    position: Arc<Mutex<usize>>,
}

#[napi]
impl VioletNoise {
    #[napi(constructor)]
    pub fn new(duration_ms: u32, sample_rate: u32, channels: u16) -> Self {
        let samples = generate_violet_noise(duration_ms, sample_rate, channels);
        Self {
            samples: Arc::new(Mutex::new(samples)),
            position: Arc::new(Mutex::new(0)),
        }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let samples = self.samples.lock().unwrap();
        samples.clone()
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        let mut pos = self.position.lock().unwrap();
        let samples = self.samples.lock().unwrap();

        if *pos >= samples.len() {
            return None;
        }
        let sample = samples[*pos];
        *pos += 1;
        Some(sample)
    }

    #[napi]
    pub fn reset(&self) {
        *self.position.lock().unwrap() = 0;
    }
}

/// White Gaussian noise
#[napi]
pub struct WhiteGaussianNoise {
    samples: Arc<Mutex<Vec<i16>>>,
    position: Arc<Mutex<usize>>,
}

#[napi]
impl WhiteGaussianNoise {
    #[napi(constructor)]
    pub fn new(duration_ms: u32, sample_rate: u32, channels: u16, std_dev: Option<f64>) -> Self {
        let samples = generate_white_gaussian_noise(duration_ms, sample_rate, channels, std_dev);
        Self {
            samples: Arc::new(Mutex::new(samples)),
            position: Arc::new(Mutex::new(0)),
        }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let samples = self.samples.lock().unwrap();
        samples.clone()
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        let mut pos = self.position.lock().unwrap();
        let samples = self.samples.lock().unwrap();

        if *pos >= samples.len() {
            return None;
        }
        let sample = samples[*pos];
        *pos += 1;
        Some(sample)
    }

    #[napi]
    pub fn reset(&self) {
        *self.position.lock().unwrap() = 0;
    }
}

/// White Triangular noise
#[napi]
pub struct WhiteTriangularNoise {
    samples: Arc<Mutex<Vec<i16>>>,
    position: Arc<Mutex<usize>>,
}

#[napi]
impl WhiteTriangularNoise {
    #[napi(constructor)]
    pub fn new(duration_ms: u32, sample_rate: u32, channels: u16) -> Self {
        let samples = generate_white_triangular_noise(duration_ms, sample_rate, channels);
        Self {
            samples: Arc::new(Mutex::new(samples)),
            position: Arc::new(Mutex::new(0)),
        }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let samples = self.samples.lock().unwrap();
        samples.clone()
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        let mut pos = self.position.lock().unwrap();
        let samples = self.samples.lock().unwrap();

        if *pos >= samples.len() {
            return None;
        }
        let sample = samples[*pos];
        *pos += 1;
        Some(sample)
    }

    #[napi]
    pub fn reset(&self) {
        *self.position.lock().unwrap() = 0;
    }
}

/// White Uniform noise (standard random noise)
#[napi]
pub struct WhiteUniformNoise {
    samples: Arc<Mutex<Vec<i16>>>,
    position: Arc<Mutex<usize>>,
}

#[napi]
impl WhiteUniformNoise {
    #[napi(constructor)]
    pub fn new(duration_ms: u32, sample_rate: u32, channels: u16) -> Self {
        let samples = generate_white_uniform_noise(duration_ms, sample_rate, channels);
        Self {
            samples: Arc::new(Mutex::new(samples)),
            position: Arc::new(Mutex::new(0)),
        }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let samples = self.samples.lock().unwrap();
        samples.clone()
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        let mut pos = self.position.lock().unwrap();
        let samples = self.samples.lock().unwrap();

        if *pos >= samples.len() {
            return None;
        }
        let sample = samples[*pos];
        *pos += 1;
        Some(sample)
    }

    #[napi]
    pub fn reset(&self) {
        *self.position.lock().unwrap() = 0;
    }
}

// =====================================================================
// Noise Generation Functions
// =====================================================================

/// Generate blue noise (high frequency emphasis)
fn generate_blue_noise(duration_ms: u32, sample_rate: u32, channels: u16) -> Vec<i16> {
    let total_samples =
        ((duration_ms as f64 * sample_rate as f64 / 1000.0) as usize) * channels as usize;
    let mut samples = Vec::with_capacity(total_samples);

    // Blue noise has more high frequency content - we'll generate uniform noise
    // and then apply a high-pass filter effect
    use rand::Rng;
    let mut rng = rand::thread_rng();

    let mut state: f64 = 0.0;

    for _ in 0..total_samples {
        // Generate random, differentiate to get high frequencies
        let input = rng.gen::<f64>() * 2.0 - 1.0; // -1 to 1
        let output = input - state;
        state = input;

        let sample = (output * 16384.0).clamp(-32768.0, 32767.0) as i16;
        samples.push(sample);
    }

    samples
}

/// Generate brownian noise (random walk)
fn generate_brownian_noise(duration_ms: u32, sample_rate: u32, channels: u16) -> Vec<i16> {
    let total_samples =
        ((duration_ms as f64 * sample_rate as f64 / 1000.0) as usize) * channels as usize;
    let mut samples = Vec::with_capacity(total_samples);

    use rand::Rng;
    let mut rng = rand::thread_rng();

    let mut state: f64 = 0.0;

    for _ in 0..total_samples {
        // Random walk - accumulate small changes
        let delta = rng.gen::<f64>() * 2.0 - 1.0; // -1 to 1
        state += delta * 0.1; // Integrate
        state = state.clamp(-1.0, 1.0); // Clamp

        let sample = (state * 32767.0) as i16;
        samples.push(sample);
    }

    samples
}

/// Generate pink noise (1/f noise)
fn generate_pink_noise(duration_ms: u32, sample_rate: u32, channels: u16) -> Vec<i16> {
    let total_samples =
        ((duration_ms as f64 * sample_rate as f64 / 1000.0) as usize) * channels as usize;
    let mut samples = Vec::with_capacity(total_samples);

    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Pink noise approximation using Paul Kellet's method
    let mut b0 = 0.0;
    let mut b1 = 0.0;
    let mut b2 = 0.0;
    let mut b3 = 0.0;
    let mut b4 = 0.0;
    let mut b5 = 0.0;
    let mut b6 = 0.0;

    for _ in 0..total_samples {
        let white = rng.gen::<f64>() * 2.0 - 1.0;

        b0 = 0.99886 * b0 + white * 0.0555179;
        b1 = 0.99332 * b1 + white * 0.0750759;
        b2 = 0.96900 * b2 + white * 0.1538520;
        b3 = 0.86650 * b3 + white * 0.3104856;
        b4 = 0.55000 * b4 + white * 0.5329522;
        b5 = -0.7616 * b5 - white * 0.0168980;

        let pink = b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362;
        b6 = white * 0.115926;

        let sample = (pink * 16384.0).clamp(-32768.0, 32767.0) as i16;
        samples.push(sample);
    }

    samples
}

/// Generate velvet noise (sparse, crackly noise)
fn generate_velvet_noise(duration_ms: u32, sample_rate: u32, channels: u16) -> Vec<i16> {
    let total_samples =
        ((duration_ms as f64 * sample_rate as f64 / 1000.0) as usize) * channels as usize;
    let mut samples = Vec::with_capacity(total_samples);

    use rand::Rng;
    let mut rng = rand::thread_rng();

    for _ in 0..total_samples {
        // Velvet noise - non-zero samples are rare and non-convex
        let sample = if rng.gen_bool(0.05) {
            let value = rng.gen::<f64>() * 2.0 - 1.0;
            (value * 32767.0) as i16
        } else {
            0
        };
        samples.push(sample);
    }

    samples
}

/// Generate violet noise (very high frequency emphasis)
fn generate_violet_noise(duration_ms: u32, sample_rate: u32, channels: u16) -> Vec<i16> {
    let total_samples =
        ((duration_ms as f64 * sample_rate as f64 / 1000.0) as usize) * channels as usize;
    let mut samples = Vec::with_capacity(total_samples);

    use rand::Rng;
    let mut rng = rand::thread_rng();

    let mut state1: f64 = 0.0;
    let mut state2: f64 = 0.0;

    for _ in 0..total_samples {
        // Violet noise - second order difference of white noise
        let input = rng.gen::<f64>() * 2.0 - 1.0;
        let diff1 = input - state1;
        let diff2 = diff1 - state2;
        state1 = input;
        state2 = diff1;

        let sample = (diff2 * 16384.0).clamp(-32768.0, 32767.0) as i16;
        samples.push(sample);
    }

    samples
}

/// Generate white Gaussian noise
fn generate_white_gaussian_noise(
    duration_ms: u32,
    sample_rate: u32,
    channels: u16,
    std_dev: Option<f64>,
) -> Vec<i16> {
    let total_samples =
        ((duration_ms as f64 * sample_rate as f64 / 1000.0) as usize) * channels as usize;
    let mut samples = Vec::with_capacity(total_samples);

    use rand::Rng;
    let mut rng = rand::thread_rng();
    let std_dev = std_dev.unwrap_or(1.0);

    for _ in 0..total_samples {
        // Box-Muller transform for Gaussian distribution
        let u1: f64 = rng.gen();
        let u2: f64 = rng.gen();
        let normal = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();

        let scaled = normal * 32767.0 * std_dev;
        let sample = scaled.clamp(-32768.0, 32767.0) as i16;
        samples.push(sample);
    }

    samples
}

/// Generate white triangular noise
fn generate_white_triangular_noise(duration_ms: u32, sample_rate: u32, channels: u16) -> Vec<i16> {
    let total_samples =
        ((duration_ms as f64 * sample_rate as f64 / 1000.0) as usize) * channels as usize;
    let mut samples = Vec::with_capacity(total_samples);

    use rand::Rng;
    let mut rng = rand::thread_rng();

    for _ in 0..total_samples {
        // Triangular distribution - sum of two uniform
        let u1 = rng.gen::<f64>() * 2.0 - 1.0;
        let u2 = rng.gen::<f64>() * 2.0 - 1.0;
        let triangular = u1 + u2;

        let sample = (triangular * 16384.0).clamp(-32768.0, 32767.0) as i16;
        samples.push(sample);
    }

    samples
}

/// Generate white uniform noise (standard uniform random)
fn generate_white_uniform_noise(duration_ms: u32, sample_rate: u32, channels: u16) -> Vec<i16> {
    let total_samples =
        ((duration_ms as f64 * sample_rate as f64 / 1000.0) as usize) * channels as usize;
    let mut samples = Vec::with_capacity(total_samples);

    use rand::Rng;
    let mut rng = rand::thread_rng();

    for _ in 0..total_samples {
        let sample = rng.gen::<i16>();
        samples.push(sample);
    }

    samples
}

// Factory functions for creating noise sources

/// Create pink noise (with 1/f frequency spectrum)
#[napi]
pub fn pink(duration_ms: u32, sample_rate: u32, channels: u16) -> PinkNoise {
    PinkNoise::new(duration_ms, sample_rate, channels)
}

/// Create white noise (neutral frequency spectrum)
#[napi]
pub fn white(duration_ms: u32, sample_rate: u32, channels: u16) -> WhiteUniformNoise {
    WhiteUniformNoise::new(duration_ms, sample_rate, channels)
}

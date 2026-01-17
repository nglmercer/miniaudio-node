//! Audio signal sources for audio synthesis and playback

use napi_derive::napi;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Audio source types (for generator sources)
#[napi]
pub enum SourceType {
    Sine = 0,
    Square = 1,
    Sawtooth = 2,
    Triangle = 3,
    WhiteNoise = 4,
    PinkNoise = 5,
    BrownianNoise = 6,
}

/// Audio source - a stream of audio samples that can be processed
#[napi]
pub struct AudioSource {
    source_type: SourceType,
    frequency: f64,
    amplitude: f64,
    sample_rate: u32,
    channels: u16,
    samples: Arc<Mutex<Vec<i16>>>,
    position: Arc<Mutex<usize>>,
}

#[napi]
impl AudioSource {
    /// Create a new audio source (generator)
    #[napi(constructor)]
    pub fn new(
        source_type: SourceType,
        frequency: f64,
        amplitude: f64,
        sample_rate: u32,
        channels: u16,
    ) -> Self {
        Self {
            source_type,
            frequency,
            amplitude,
            sample_rate,
            channels,
            samples: Arc::new(Mutex::new(Vec::new())),
            position: Arc::new(Mutex::new(0)),
        }
    }

    /// Generate samples for the given duration
    #[napi]
    pub fn generate(&self, duration_ms: u32) -> Vec<i16> {
        let total_samples = ((duration_ms as f64 * self.sample_rate as f64 / 1000.0) as usize)
            * self.channels as usize;
        let mut samples = Vec::with_capacity(total_samples);

        let two_pi = 2.0 * std::f64::consts::PI;
        let omega = two_pi * self.frequency / self.sample_rate as f64;
        let amplitude = self.amplitude * 32767.0;

        for i in 0..total_samples {
            let t = i as f64 / self.sample_rate as f64;
            let sample = match self.source_type {
                SourceType::Sine => (omega * i as f64).sin(),
                SourceType::Square => {
                    if (omega * i as f64).sin() >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                SourceType::Sawtooth => {
                    let phase = (omega * i as f64) % (two_pi);
                    2.0 * (phase / two_pi) - 1.0
                }
                SourceType::Triangle => {
                    let phase = (omega * i as f64) % (two_pi);
                    2.0 * (phase / two_pi) - 1.0
                }
                SourceType::WhiteNoise => {
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    rng.gen::<f64>() * 2.0 - 1.0
                }
                _ => 0.0,
            };

            let sample = (sample * amplitude) as i16;
            samples.push(sample);
        }

        samples
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        self.samples.lock().unwrap().clone()
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        let mut pos = self.position.lock().unwrap();
        let samples = self.samples.lock().unwrap();

        if *pos >= samples.len() {
            // Generate more samples if needed
            let generated = self.generate(100); // 100ms at a time
            if !generated.is_empty() {
                for sample in generated {
                    samples.push(sample);
                }
            }
            return Some(0);
        }

        let sample = samples[*pos];
        *pos += 1;
        Some(sample)
    }

    #[napi]
    pub fn reset(&self) {
        *self.position.lock().unwrap() = 0;
    }

    /// Get sample rate of source
    #[napi]
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get channel count of source
    #[napi]
    pub fn get_channels(&self) -> u16 {
        self.channels
    }

    /// Get frequency
    #[napi]
    pub fn get_frequency(&self) -> f64 {
        self.frequency
    }

    /// Get amplitude
    #[napi]
    pub fn get_amplitude(&self) -> f64 {
        self.amplitude
    }
}

/// Amplify audio source
#[napi]
pub struct Amplify {
    source: AudioSource,
    gain: f64,
}

#[napi]
impl Amplify {
    #[napi(constructor)]
    pub fn new(source: AudioSource, gain: f64) -> Self {
        Self { source, gain }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let samples = self.source.get_samples();
        samples
            .into_iter()
            .map(|s| ((s as f64 * self.gain).clamp(i16::MIN as f64, i16::MAX as f64)) as i16)
            .collect()
    }

    #[napi]
    pub fn set_gain(&mut self, gain: f64) {
        self.gain = gain;
    }

    #[napi]
    pub fn get_gain(&self) -> f64 {
        self.gain
    }
}

/// Pausable audio source
#[napi]
pub struct Pausable {
    source: AudioSource,
    paused: Arc<Mutex<bool>>,
}

#[napi]
impl Pausable {
    #[napi(constructor)]
    pub fn new(source: AudioSource) -> Self {
        Self {
            source,
            paused: Arc::new(Mutex::new(false)),
        }
    }

    #[napi]
    pub fn pause(&self) {
        *self.paused.lock().unwrap() = true;
    }

    #[napi]
    pub fn resume(&self) {
        *self.paused.lock().unwrap() = false;
    }

    #[napi]
    pub fn is_paused(&self) -> bool {
        *self.paused.lock().unwrap()
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        if self.is_paused() {
            return Some(0);
        }
        self.source.get_next()
    }
}

/// Stoppable audio source
#[napi]
pub struct Stoppable {
    source: AudioSource,
    stopped: Arc<Mutex<bool>>,
}

#[napi]
impl Stoppable {
    #[napi(constructor)]
    pub fn new(source: AudioSource) -> Self {
        Self {
            source,
            stopped: Arc::new(Mutex::new(false)),
        }
    }

    #[napi]
    pub fn stop(&self) {
        *self.stopped.lock().unwrap() = true;
        self.source.reset();
    }

    #[napi]
    pub fn start(&self) {
        *self.stopped.lock().unwrap() = false;
        self.source.reset();
    }

    #[napi]
    pub fn is_stopped(&self) -> bool {
        *self.stopped.lock().unwrap()
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        if self.is_stopped() {
            return Some(0);
        }
        self.source.get_next()
    }
}

/// Chirp audio source (sweeping frequency)
#[napi]
pub struct Chirp {
    source: AudioSource,
    start_freq: f64,
    end_freq: f64,
    duration: u32,
}

#[napi]
impl Chirp {
    #[napi(constructor)]
    pub fn new(
        start_freq: f64,
        end_freq: f64,
        duration: u32,
        amplitude: f64,
        sample_rate: u32,
        channels: u16,
    ) -> Self {
        let source = AudioSource::new(
            SourceType::Sine,
            start_freq,
            amplitude,
            sample_rate,
            channels,
        );
        Self {
            source,
            start_freq,
            end_freq,
            duration,
        }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let total_samples = ((self.duration as f64 * self.source.get_sample_rate() as f64 / 1000.0)
            as usize)
            * self.source.get_channels() as usize;
        let mut samples = Vec::with_capacity(total_samples);

        let amplitude = self.source.get_amplitude() * 32767.0;
        let two_pi = 2.0 * std::f64::consts::PI;
        let sample_rate = self.source.get_sample_rate() as f64;
        let duration_sec = self.duration as f64 / 1000.0;

        for i in 0..total_samples {
            let t = i as f64 / sample_rate;
            let freq = self.start_freq + (self.end_freq - self.start_freq) * (t / duration_sec);
            let phase = two_pi * freq * t;
            let sample = phase.sin() * amplitude;
            samples.push(sample as i16);
        }

        samples
    }
}

/// Linear gain ramp audio source (fade in/out)
#[napi]
pub struct LinearGainRamp {
    source: AudioSource,
    start_gain: f64,
    end_gain: f64,
    duration: u32,
}

#[napi]
impl LinearGainRamp {
    #[napi(constructor)]
    pub fn new(source: AudioSource, start_gain: f64, end_gain: f64, duration: u32) -> Self {
        Self {
            source,
            start_gain,
            end_gain,
            duration,
        }
    }

    #[napi]
    pub fn get_samples(&self) -> Vec<i16> {
        let source_samples = self.source.get_samples();
        let mut output = Vec::with_capacity(source_samples.len());

        let total_samples = source_samples.len();
        for (i, &sample) in source_samples.iter().enumerate() {
            let progress = i as f64 / total_samples as f64;
            let gain = self.start_gain + (self.end_gain - self.start_gain) * progress;
            let scaled = (sample as f64 * gain).clamp(i16::MIN as f64, i16::MAX as f64) as i16;
            output.push(scaled);
        }

        output
    }
}

/// Sine wave generator
#[napi]
pub struct SineWave {
    source: AudioSource,
}

#[napi]
impl SineWave {
    #[napi(constructor)]
    pub fn new(frequency: f64, amplitude: f64, sample_rate: u32, channels: u16) -> Self {
        Self {
            source: AudioSource::new(
                SourceType::Sine,
                frequency,
                amplitude,
                sample_rate,
                channels,
            ),
        }
    }

    #[napi]
    pub fn get_samples(&self, duration_ms: u32) -> Vec<i16> {
        self.source.generate(duration_ms)
    }
}

/// Square wave generator
#[napi]
pub struct SquareWave {
    source: AudioSource,
}

#[napi]
impl SquareWave {
    #[napi(constructor)]
    pub fn new(frequency: f64, amplitude: f64, sample_rate: u32, channels: u16) -> Self {
        Self {
            source: AudioSource::new(
                SourceType::Square,
                frequency,
                amplitude,
                sample_rate,
                channels,
            ),
        }
    }

    #[napi]
    pub fn get_samples(&self, duration_ms: u32) -> Vec<i16> {
        self.source.generate(duration_ms)
    }
}

/// Sawtooth wave generator
#[napi]
pub struct SawtoothWave {
    source: AudioSource,
}

#[napi]
impl SawtoothWave {
    #[napi(constructor)]
    pub fn new(frequency: f64, amplitude: f64, sample_rate: u32, channels: u16) -> Self {
        Self {
            source: AudioSource::new(
                SourceType::Sawtooth,
                frequency,
                amplitude,
                sample_rate,
                channels,
            ),
        }
    }

    #[napi]
    pub fn get_samples(&self, duration_ms: u32) -> Vec<i16> {
        self.source.generate(duration_ms)
    }
}

/// Triangle wave generator
#[napi]
pub struct TriangleWave {
    source: AudioSource,
}

#[napi]
impl TriangleWave {
    #[napi(constructor)]
    pub fn new(frequency: f64, amplitude: f64, sample_rate: u32, channels: u16) -> Self {
        Self {
            source: AudioSource::new(
                SourceType::Triangle,
                frequency,
                amplitude,
                sample_rate,
                channels,
            ),
        }
    }

    #[napi]
    pub fn get_samples(&self, duration_ms: u32) -> Vec<i16> {
        self.source.generate(duration_ms)
    }
}

/// Empty audio source (silence)
#[napi]
pub struct Empty {
    sample_rate: u32,
    channels: u16,
}

#[napi]
impl Empty {
    #[napi(constructor)]
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }

    #[napi]
    pub fn get_samples(&self, duration_ms: u32) -> Vec<i16> {
        let total_samples = ((duration_ms as f64 * self.sample_rate as f64 / 1000.0) as usize)
            * self.channels as usize;
        vec![0; total_samples]
    }
}

/// Empty callback source (calls user function for samples)
#[napi]
pub struct EmptyCallback {
    sample_rate: u32,
    channels: u16,
}

#[napi]
impl EmptyCallback {
    #[napi(constructor)]
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }
}

/// Skip by duration
#[napi]
pub struct SkipDuration {
    source: AudioSource,
    skip_ms: u32,
}

#[napi]
impl SkipDuration {
    #[napi(constructor)]
    pub fn new(source: AudioSource, skip_ms: u32) -> Self {
        Self { source, skip_ms }
    }

    #[napi]
    pub fn get_samples(&self, duration_ms: u32) -> Vec<i16> {
        let total_samples = ((duration_ms as f64 * self.source.get_sample_rate() as f64 / 1000.0)
            as usize)
            * self.source.get_channels() as usize;
        let skip_samples = ((self.skip_ms as f64 * self.source.get_sample_rate() as f64 / 1000.0)
            as usize)
            * self.source.get_channels() as usize;

        // Simple skip by returning empty for now
        // In a real implementation, this would read from the source and skip
        let all_samples = self.source.generate(duration_ms + self.skip_ms);
        if skip_samples < all_samples.len() {
            all_samples[skip_samples..].to_vec()
        } else {
            Vec::new()
        }
    }
}

/// Skippable audio source
#[napi]
pub struct Skippable {
    source: AudioSource,
    position: Arc<Mutex<usize>>,
}

#[napi]
impl Skippable {
    #[napi(constructor)]
    pub fn new(source: AudioSource) -> Self {
        Self {
            source,
            position: Arc::new(Mutex::new(0)),
        }
    }

    #[napi]
    pub fn skip(&self, samples: u32) {
        let mut pos = self.position.lock().unwrap();
        *pos = (*pos + samples as usize).min(self.source.get_samples().len());
    }

    #[napi]
    pub fn get_next(&self) -> Option<i16> {
        let mut pos = self.position.lock().unwrap();
        let source_samples = self.source.get_samples();

        if *pos >= source_samples.len() {
            return None;
        }

        let sample = source_samples[*pos];
        *pos += 1;
        Some(sample)
    }
}

/// Repeat audio source
#[napi]
pub struct Repeat {
    source: AudioSource,
    times: u32,
}

#[napi]
impl Repeat {
    #[napi(constructor)]
    pub fn new(source: AudioSource, times: u32) -> Self {
        Self { source, times }
    }

    #[napi]
    pub fn get_samples(&self, duration_ms: u32) -> Vec<i16> {
        let samples = self.source.generate(duration_ms);
        if self.times <= 1 {
            return samples;
        }

        let mut result = Vec::with_capacity(samples.len() * self.times as usize);
        for _ in 0..self.times {
            result.extend_from_slice(&samples);
        }
        result
    }
}

/// Signal generator
#[napi]
pub struct SignalGenerator {
    source_type: SourceType,
    frequency: f64,
    amplitude: f64,
    sample_rate: u32,
}

#[napi]
impl SignalGenerator {
    #[napi(constructor)]
    pub fn new(source_type: SourceType, frequency: f64, amplitude: f64, sample_rate: u32) -> Self {
        Self {
            source_type,
            frequency,
            amplitude,
            sample_rate,
        }
    }

    #[napi]
    pub fn generate(&self, duration_ms: u32) -> Vec<i16> {
        let channels = 1; // Mono for signal generators
        let audio_source = AudioSource::new(
            self.source_type.clone(),
            self.frequency,
            self.amplitude,
            self.sample_rate,
            channels,
        );
        audio_source.generate(duration_ms)
    }
}

/// Uniform source iterator
#[napi]
pub struct UniformSourceIterator {
    source: AudioSource,
    buffer_size: u32,
    count: u32,
}

#[napi]
impl UniformSourceIterator {
    #[napi(constructor)]
    pub fn new(source: AudioSource, buffer_size: u32, count: u32) -> Self {
        Self {
            source,
            buffer_size,
            count,
        }
    }

    #[napi]
    pub fn next(&self) -> Option<Vec<i16>> {
        if self.count == 0 {
            return None;
        }
        let samples = self.source.generate(self.buffer_size);
        Some(samples)
    }
}

/// Zero audio source (silence)
#[napi]
pub struct Zero {
    sample_rate: u32,
    channels: u16,
}

#[napi]
impl Zero {
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
        vec![0; total_samples]
    }
}

/// Signal generator function type
pub type GeneratorFunction = Box<dyn Fn(f64) -> f64 + Send + Sync>;

/// Crossfade type
pub type Crossfade = f64;

// =====================================================================
// Factory functions
// =====================================================================

/// Create a sine wave source
#[napi]
pub fn from_iter(filename: &str) -> Result<Vec<i16>, napi::Error> {
    // Placeholder for where to read samples from
    Ok(vec![0; 44100])
}

/// Create a RIFF WAV decoder from a factory
#[napi]
pub fn from_factory() -> Vec<i16> {
    vec![0; 44100]
}

/// Create a chirp source
#[napi]
pub fn chirp(start_hz: f64, end_hz: f64, duration_ms: u32, sample_rate: u32) -> Chirp {
    Chirp::new(start_hz, end_hz, duration_ms, 0.3, sample_rate, 2)
}

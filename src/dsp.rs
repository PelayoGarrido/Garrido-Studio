use crate::model::Effect;
use std::f64::consts::{PI, TAU};

pub fn pan(value: f32) -> [f32; 2] {
    let value = if value.is_finite() { value } else { 0. };
    let angle = (value.clamp(-1., 1.) as f64 + 1.) * PI / 4.;
    [angle.cos() as f32, angle.sin() as f32]
}

pub fn poly_blep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.
    } else if t > 1. - dt {
        let x = (t - 1.) / dt;
        x * x + x + x + 1.
    } else {
        0.
    }
}

pub fn saw(phase: f64, step: f64) -> f64 {
    2. * phase - 1. - poly_blep(phase, step)
}

pub fn square(phase: f64, step: f64) -> f64 {
    (if phase < 0.5 { 1. } else { -1. }) + poly_blep(phase, step)
        - poly_blep((phase + 0.5).fract(), step)
}

pub fn noise(state: &mut u64) -> f64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 11) as f64 / ((1u64 << 53) - 1) as f64 * 2. - 1.
}

pub fn lerp_sample(frames: &[[f32; 2]], position: f64) -> [f32; 2] {
    if frames.is_empty()
        || !position.is_finite()
        || position < 0.
        || position >= frames.len() as f64
    {
        return [0.; 2];
    }
    let i = position as usize;
    let f = (position - i as f64) as f32;
    let a = frames[i];
    let b = frames.get(i + 1).copied().unwrap_or([0.; 2]);
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
}

pub fn sinc_sample(frames: &[[f32; 2]], position: f64, ratio: f64) -> [f32; 2] {
    if !position.is_finite()
        || !ratio.is_finite()
        || ratio <= 0.
        || position < 0.
        || position >= frames.len() as f64
    {
        return [0.; 2];
    }
    let cutoff = (1. / ratio).min(1.) * 0.94;
    let center = position.floor() as i64;
    let mut result = [0.; 2];
    let mut total = 0.;
    for tap in -15..=16 {
        let index = center + tap;
        let distance = position - index as f64;
        if distance.abs() >= 16. {
            continue;
        }
        let x = PI * distance * cutoff;
        let weight = (if x.abs() < 1e-12 { 1. } else { x.sin() / x })
            * (0.5 + 0.5 * (PI * distance / 16.).cos())
            * cutoff;
        total += weight;
        if let Some(frame) = usize::try_from(index).ok().and_then(|i| frames.get(i)) {
            result[0] += frame[0] as f64 * weight;
            result[1] += frame[1] as f64 * weight;
        }
    }
    if total.abs() < 1e-12 {
        return [0.; 2];
    }
    [(result[0] / total) as f32, (result[1] / total) as f32]
}

#[derive(Clone)]
struct DelayLine {
    data: Vec<[f32; 2]>,
    cursor: usize,
}
impl DelayLine {
    fn new(size: usize) -> Self {
        Self {
            data: vec![[0.; 2]; size.max(2)],
            cursor: 0,
        }
    }
    fn current(&self) -> [f32; 2] {
        self.data[self.cursor]
    }
    fn push(&mut self, value: [f32; 2]) {
        self.data[self.cursor] = value;
        self.cursor = (self.cursor + 1) % self.data.len();
    }
    fn behind(&self, delay: f64) -> [f32; 2] {
        let at = (self.cursor as f64 - delay).rem_euclid(self.data.len() as f64);
        let i = at as usize;
        let fraction = (at - i as f64) as f32;
        let a = self.data[i];
        let b = self.data[(i + 1) % self.data.len()];
        [
            a[0] + (b[0] - a[0]) * fraction,
            a[1] + (b[1] - a[1]) * fraction,
        ]
    }
}

#[derive(Clone)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [[f64; 2]; 2],
}
impl Biquad {
    fn peak(rate: u32, hz: f32, db: f32, q: f32) -> Self {
        let w = TAU * (hz as f64).min(rate as f64 * 0.45) / rate as f64;
        let alpha = w.sin() / (2. * q as f64);
        let a = 10f64.powf(db as f64 / 40.);
        let a0 = 1. + alpha / a;
        Self {
            b: [
                (1. + alpha * a) / a0,
                -2. * w.cos() / a0,
                (1. - alpha * a) / a0,
            ],
            a: [-2. * w.cos() / a0, (1. - alpha / a) / a0],
            z: [[0.; 2]; 2],
        }
    }
    fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        std::array::from_fn(|i| {
            let x = input[i] as f64;
            let y = self.b[0] * x + self.z[i][0];
            self.z[i][0] = clean(self.b[1] * x - self.a[0] * y + self.z[i][1]);
            self.z[i][1] = clean(self.b[2] * x - self.a[1] * y);
            clean(y) as f32
        })
    }
}

fn clean(value: f64) -> f64 {
    if !value.is_finite() || value.abs() < 1e-25 {
        0.
    } else {
        value.clamp(-64., 64.)
    }
}
fn blend(a: [f32; 2], b: [f32; 2], mix: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * mix, a[1] + (b[1] - a[1]) * mix]
}

#[derive(Clone)]
enum State {
    Eq(Biquad),
    Delay(DelayLine),
    Reverb([DelayLine; 4]),
    Chorus { line: DelayLine, phase: f64 },
    Compressor(f32),
    Stateless,
}

#[derive(Clone)]
pub struct Processor {
    spec: Effect,
    state: State,
    rate: u32,
}
impl Processor {
    pub fn reset(&mut self) {
        match &mut self.state {
            State::Eq(eq) => eq.z = [[0.; 2]; 2],
            State::Delay(line) => {
                line.data.fill([0.; 2]);
                line.cursor = 0;
            }
            State::Reverb(lines) => {
                for line in lines {
                    line.data.fill([0.; 2]);
                    line.cursor = 0;
                }
            }
            State::Chorus { line, phase } => {
                line.data.fill([0.; 2]);
                line.cursor = 0;
                *phase = 0.;
            }
            State::Compressor(value) => *value = 0.,
            State::Stateless => {}
        }
    }
    pub fn new(spec: &Effect, rate: u32) -> crate::Result<Self> {
        spec.validate()?;
        if !(8_000..=192_000).contains(&rate) {
            return Err(crate::Error::Invalid("Frecuencia no válida"));
        }
        let state = match *spec {
            Effect::Eq { hz, db, q } => State::Eq(Biquad::peak(rate, hz, db, q)),
            Effect::Delay { seconds, .. } => {
                State::Delay(DelayLine::new((seconds * rate as f32).round() as usize))
            }
            Effect::Reverb { .. } => State::Reverb(
                [0.0297, 0.0371, 0.0411, 0.0437]
                    .map(|seconds| DelayLine::new((seconds * rate as f64).round() as usize)),
            ),
            Effect::Chorus { .. } => State::Chorus {
                line: DelayLine::new((rate as f64 * 0.04) as usize),
                phase: 0.,
            },
            Effect::Compressor { .. } => State::Compressor(0.),
            _ => State::Stateless,
        };
        Ok(Self {
            spec: spec.clone(),
            state,
            rate,
        })
    }
    pub fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let output = match (&self.spec, &mut self.state) {
            (Effect::Eq { .. }, State::Eq(eq)) => eq.process(x),
            (Effect::Delay { feedback, mix, .. }, State::Delay(line)) => {
                let old = line.current();
                line.push([x[0] + old[0] * feedback, x[1] + old[1] * feedback]);
                blend(x, old, *mix)
            }
            (Effect::Reverb { decay, mix }, State::Reverb(lines)) => {
                let mut wet = [0.; 2];
                for (i, line) in lines.iter_mut().enumerate() {
                    let old = line.current();
                    let factor = decay * (0.85 + i as f32 * 0.035);
                    line.push([x[0] + old[1] * factor, x[1] + old[0] * factor]);
                    wet[0] += old[0] * 0.25;
                    wet[1] += old[1] * 0.25;
                }
                blend(x, wet, *mix)
            }
            (Effect::Chorus { rate, depth, mix }, State::Chorus { line, phase }) => {
                let delay = self.rate as f64 * (0.015 + 0.008 * (*depth as f64) * phase.sin());
                let wet = line.behind(delay);
                line.push(x);
                *phase = (*phase + TAU * (*rate as f64) / self.rate as f64) % TAU;
                blend(x, wet, *mix)
            }
            (Effect::Drive { drive, mix }, _) => blend(
                x,
                [
                    (x[0] * drive).tanh() / drive.sqrt(),
                    (x[1] * drive).tanh() / drive.sqrt(),
                ],
                *mix,
            ),
            (Effect::Width(width), _) => {
                let mid = (x[0] + x[1]) * 0.5;
                let side = (x[0] - x[1]) * 0.5 * width;
                [mid + side, mid - side]
            }
            (Effect::Compressor { threshold, ratio }, State::Compressor(envelope)) => {
                let peak = x[0].abs().max(x[1].abs());
                let seconds = if peak > *envelope { 0.003 } else { 0.08 };
                let coefficient = (-1. / (seconds * self.rate as f32)).exp();
                *envelope = peak + coefficient * (*envelope - peak);
                let gain = if *envelope > *threshold {
                    (*threshold / *envelope).powf(1. - 1. / ratio)
                } else {
                    1.
                };
                [x[0] * gain, x[1] * gain]
            }
            _ => x,
        };
        output.map(|v| clean(v as f64) as f32)
    }
}

pub struct OutputGuard {
    last_x: [f32; 2],
    last_y: [f32; 2],
    coefficient: f32,
}
impl OutputGuard {
    pub fn new(rate: u32) -> Self {
        Self {
            last_x: [0.; 2],
            last_y: [0.; 2],
            coefficient: (-TAU * 15. / rate as f64).exp() as f32,
        }
    }
    pub fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        std::array::from_fn(|i| {
            let x = if input[i].is_finite() {
                input[i].clamp(-64., 64.)
            } else {
                0.
            };
            let y = x - self.last_x[i] + self.coefficient * self.last_y[i];
            self.last_x[i] = x;
            self.last_y[i] = if y.abs() < 1e-20 { 0. } else { y };
            y.clamp(-0.8, 0.8)
        })
    }
}

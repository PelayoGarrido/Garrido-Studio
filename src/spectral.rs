use crate::{Error, Result, model::Sample};
use std::{
    f64::consts::TAU,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const MAX_MODES: usize = 32;
pub const MAX_SECONDS: f64 = 12.;

#[derive(Clone, Copy, Debug)]
pub struct Mode {
    pub hz: f64,
    pub power: f64,
    pub left: f64,
}

#[derive(Clone, Debug)]
pub struct Profile {
    modes: Vec<Mode>,
    rms: f64,
}

impl Profile {
    pub fn modes(&self) -> &[Mode] {
        &self.modes
    }
    pub fn rms(&self) -> f64 {
        self.rms
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Route {
    pub from_hz: f64,
    pub to_hz: f64,
    pub power: f64,
    left_a: f64,
    left_b: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub rate: u32,
    pub seconds: f64,
    pub start: f64,
    pub end: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rate: 48_000,
            seconds: 6.,
            start: 0.,
            end: 1.,
        }
    }
}

impl Settings {
    pub fn validate(self) -> Result<()> {
        if !(8_000..=192_000).contains(&self.rate)
            || !self.seconds.is_finite()
            || !(0.25..=MAX_SECONDS).contains(&self.seconds)
            || !self.start.is_finite()
            || !(0. ..=1.).contains(&self.start)
            || !self.end.is_finite()
            || !(0. ..=1.).contains(&self.end)
        {
            return Err(Error::Invalid(
                "Spectral Journey: parámetros fuera de rango",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    fn power(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

fn fft(data: &mut [Complex]) {
    let n = data.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            data.swap(i, j);
        }
    }
    let mut length = 2;
    while length <= n {
        let angle = -TAU / length as f64;
        let step = Complex {
            re: angle.cos(),
            im: angle.sin(),
        };
        for block in data.chunks_exact_mut(length) {
            let mut phase = Complex { re: 1., im: 0. };
            let (a, b) = block.split_at_mut(length / 2);
            for (a, b) in a.iter_mut().zip(b) {
                let next = Complex {
                    re: b.re * phase.re - b.im * phase.im,
                    im: b.re * phase.im + b.im * phase.re,
                };
                *b = Complex {
                    re: a.re - next.re,
                    im: a.im - next.im,
                };
                a.re += next.re;
                a.im += next.im;
                phase = Complex {
                    re: phase.re * step.re - phase.im * step.im,
                    im: phase.re * step.im + phase.im * step.re,
                };
            }
        }
        length *= 2;
    }
}

fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(Error::Invalid("Spectral Journey cancelado"));
    }
    Ok(())
}

pub fn analyze(sample: &Sample, modes: usize, cancel: &AtomicBool) -> Result<Profile> {
    cancelled(cancel)?;
    if !(1..=MAX_MODES).contains(&modes)
        || !(8_000..=192_000).contains(&sample.rate)
        || sample.frames.len() < 128
        || sample.frames.len() as f64 > sample.rate as f64 * MAX_SECONDS
    {
        return Err(Error::Limit(
            "Usa entre 128 frames y 12 segundos de audio; máximo 32 modos",
        ));
    }
    let mut mean = [0.; 2];
    for (index, frame) in sample.frames.iter().enumerate() {
        if index % 4096 == 0 {
            cancelled(cancel)?;
        }
        for (mean, &value) in mean.iter_mut().zip(frame) {
            if !value.is_finite() || value.abs() > 16. {
                return Err(Error::Invalid("Spectral Journey: audio no válido"));
            }
            *mean += value as f64 / sample.frames.len() as f64;
        }
    }
    let energy: f64 = sample
        .frames
        .iter()
        .map(|frame| {
            frame
                .iter()
                .zip(mean)
                .map(|(&v, m)| (v as f64 - m).powi(2))
                .sum::<f64>()
        })
        .sum();
    let rms = (energy / (sample.frames.len() * 2) as f64).sqrt();
    if rms < 1e-7 {
        return Err(Error::Invalid(
            "La fuente no contiene señal alterna suficiente",
        ));
    }
    let size = (sample.rate as usize / 20)
        .next_power_of_two()
        .clamp(1024, 8192);
    let mut bins = vec![[0.; 2]; size / 2];
    let mut buffer = vec![Complex::default(); size];
    let window: Vec<_> = (0..size)
        .map(|i| 0.5 - 0.5 * (TAU * i as f64 / size as f64).cos())
        .collect();
    let available = sample.frames.len().saturating_sub(size);
    let windows = if available == 0 { 1 } else { 32 };
    for index in 0..windows {
        cancelled(cancel)?;
        let start = if windows == 1 {
            0
        } else {
            available * index / (windows - 1)
        };
        for channel in 0..2 {
            for (i, value) in buffer.iter_mut().enumerate() {
                let input = sample
                    .frames
                    .get(start + i)
                    .map_or(0., |f| f[channel] as f64 - mean[channel]);
                *value = Complex {
                    re: input * window[i],
                    im: 0.,
                };
            }
            fft(&mut buffer);
            for (bin, value) in bins.iter_mut().zip(&buffer) {
                bin[channel] += value.power() / windows as f64;
            }
        }
    }
    let power = |i: usize| bins[i][0] + bins[i][1];
    let mut peaks = Vec::new();
    for (i, bin) in bins.iter().enumerate().take(bins.len() - 1).skip(2) {
        let center = power(i);
        if center <= 1e-16 || center <= power(i - 1) || center < power(i + 1) {
            continue;
        }
        let a = power(i - 1).max(1e-30).ln();
        let b = center.ln();
        let c = power(i + 1).max(1e-30).ln();
        let denominator = a - 2. * b + c;
        let offset = if denominator.abs() < 1e-15 {
            0.
        } else {
            (0.5 * (a - c) / denominator).clamp(-0.5, 0.5)
        };
        let hz = (i as f64 + offset) * sample.rate as f64 / size as f64;
        if hz >= 20. && hz < sample.rate as f64 * 0.45 {
            peaks.push(Mode {
                hz,
                power: center,
                left: bin[0] / center,
            });
        }
    }
    peaks.sort_by(|a, b| b.power.total_cmp(&a.power).then(a.hz.total_cmp(&b.hz)));
    let maximum = peaks.first().map_or(0., |m| m.power);
    peaks.retain(|m| m.power >= maximum * 0.0025);
    peaks.truncate(modes);
    if peaks.is_empty() {
        return Err(Error::Invalid(
            "No hay modos audibles suficientes para construir el recorrido",
        ));
    }
    let total: f64 = peaks.iter().map(|m| m.power).sum();
    for mode in &mut peaks {
        mode.power /= total;
    }
    peaks.sort_by(|a, b| a.hz.total_cmp(&b.hz));
    Ok(Profile { modes: peaks, rms })
}

pub fn routes(a: &Profile, b: &Profile) -> Vec<Route> {
    let mut result = Vec::with_capacity(a.modes.len() + b.modes.len());
    let (mut i, mut j) = (0, 0);
    let (mut left, mut right) = (a.modes[0].power, b.modes[0].power);
    while i < a.modes.len() && j < b.modes.len() {
        let power = left.min(right);
        if power > 1e-14 {
            result.push(Route {
                from_hz: a.modes[i].hz,
                to_hz: b.modes[j].hz,
                power,
                left_a: a.modes[i].left,
                left_b: b.modes[j].left,
            });
        }
        left -= power;
        right -= power;
        if left < 1e-14 {
            i += 1;
            if let Some(mode) = a.modes.get(i) {
                left = mode.power;
            }
        }
        if right < 1e-14 {
            j += 1;
            if let Some(mode) = b.modes.get(j) {
                right = mode.power;
            }
        }
    }
    result
}

pub fn render(a: &Profile, b: &Profile, settings: Settings, cancel: &AtomicBool) -> Result<Sample> {
    settings.validate()?;
    cancelled(cancel)?;
    let routes = routes(a, b);
    if routes
        .iter()
        .any(|r| r.from_hz.max(r.to_hz) >= settings.rate as f64 * 0.45)
    {
        return Err(Error::Invalid(
            "La frecuencia de salida es insuficiente para estos modos",
        ));
    }
    let size = (settings.rate as f64 * settings.seconds).round() as usize;
    let mut frames = vec![[0f32; 2]; size];
    let mut phases: Vec<_> = (0..routes.len())
        .map(|i| (i as f64 * 2.399963229728653).rem_euclid(TAU))
        .collect();
    let logs: Vec<_> = routes
        .iter()
        .map(|r| (r.from_hz.ln(), (r.to_hz / r.from_hz).ln()))
        .collect();
    let fade = ((settings.rate as f64 * 0.01) as usize)
        .min(size / 4)
        .max(1);
    let mut peak = 0f64;
    for (index, frame) in frames.iter_mut().enumerate() {
        if index % 256 == 0 {
            cancelled(cancel)?;
        }
        let position = index as f64 / (size - 1) as f64;
        let progress = position * position * (3. - 2. * position);
        let amount = settings.start + (settings.end - settings.start) * progress;
        let rms = (a.rms + (b.rms - a.rms) * amount).min(0.25);
        let envelope = (index.min(size - 1 - index) as f64 / fade as f64).min(1.);
        let envelope = (envelope * std::f64::consts::FRAC_PI_2).sin().powi(2);
        let mut value = [0.; 2];
        for ((route, phase), &(log_a, delta)) in routes.iter().zip(&mut phases).zip(&logs) {
            let hz = (log_a + amount * delta).exp();
            let wave = phase.sin() * 2. * rms * route.power.sqrt();
            let pan = (route.left_a + amount * (route.left_b - route.left_a)).clamp(0., 1.);
            value[0] += wave * pan.sqrt();
            value[1] += wave * (1. - pan).sqrt();
            *phase = (*phase + TAU * hz / settings.rate as f64).rem_euclid(TAU);
        }
        for (out, value) in frame.iter_mut().zip(value) {
            *out = (value * envelope) as f32;
            peak = peak.max(out.abs() as f64);
        }
    }
    if peak > 0.85 {
        let gain = (0.85 / peak) as f32;
        for frame in &mut frames {
            frame[0] *= gain;
            frame[1] *= gain;
        }
    }
    cancelled(cancel)?;
    Ok(Sample {
        rate: settings.rate,
        frames: Arc::new(frames),
    })
}

pub struct Comparison {
    pub journey: Sample,
    pub crossfade: Sample,
    pub routes: Vec<Route>,
}

pub fn compare(
    a: &Profile,
    b: &Profile,
    settings: Settings,
    cancel: &AtomicBool,
) -> Result<Comparison> {
    let mut journey = render(a, b, settings, cancel)?;
    let endpoint = Settings {
        start: 0.,
        end: 0.,
        ..settings
    };
    let mut crossfade = render(a, a, endpoint, cancel)?;
    let other = render(b, b, endpoint, cancel)?;
    for (index, (frame, other)) in Arc::make_mut(&mut crossfade.frames)
        .iter_mut()
        .zip(other.frames.iter())
        .enumerate()
    {
        if index % 4096 == 0 {
            cancelled(cancel)?;
        }
        let position = index as f64 / (journey.frames.len() - 1) as f64;
        let amount = settings.start
            + (settings.end - settings.start) * position * position * (3. - 2. * position);
        for (value, &other) in frame.iter_mut().zip(other) {
            *value += (other - *value) * amount as f32;
        }
    }
    let rms = |s: &Sample| {
        (s.frames
            .iter()
            .flatten()
            .map(|v| (*v as f64).powi(2))
            .sum::<f64>()
            / (s.frames.len() * 2) as f64)
            .sqrt()
    };
    let target = rms(&journey).min(rms(&crossfade));
    for sample in [&mut journey, &mut crossfade] {
        let measured = rms(sample);
        if measured > 1e-12 {
            let gain = (target / measured).min(1.) as f32;
            for frame in Arc::make_mut(&mut sample.frames) {
                frame[0] *= gain;
                frame[1] *= gain;
            }
        }
    }
    cancelled(cancel)?;
    Ok(Comparison {
        journey,
        crossfade,
        routes: routes(a, b),
    })
}

pub fn example_sources() -> [Sample; 2] {
    [[220., 440., 660., 880.], [330., 495., 825., 1320.]].map(|frequencies| Sample {
        rate: 48_000,
        frames: Arc::new(
            (0..24_000)
                .map(|i| {
                    let value = frequencies
                        .iter()
                        .enumerate()
                        .map(|(j, hz)| {
                            (TAU * hz * i as f64 / 48_000. + j as f64 * 2.399963229728653).sin()
                                * 0.075
                        })
                        .sum::<f64>() as f32;
                    [value; 2]
                })
                .collect(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f64, left: f64) -> Sample {
        Sample {
            rate: 48_000,
            frames: Arc::new(
                (0..24_000)
                    .map(|i| {
                        let x = (TAU * hz * i as f64 / 48_000.).sin() * 0.2;
                        [(x * left.sqrt()) as f32, (x * (1. - left).sqrt()) as f32]
                    })
                    .collect(),
            ),
        }
    }

    #[test]
    fn matched_comparison_does_not_win_by_being_louder() {
        let cancel = AtomicBool::new(false);
        let [a, b] = example_sources();
        let a = analyze(&a, 24, &cancel).unwrap();
        let b = analyze(&b, 24, &cancel).unwrap();
        let result = compare(
            &a,
            &b,
            Settings {
                seconds: 0.5,
                ..Default::default()
            },
            &cancel,
        )
        .unwrap();
        let rms = |s: &Sample| {
            (s.frames
                .iter()
                .flatten()
                .map(|v| (*v as f64).powi(2))
                .sum::<f64>()
                / (s.frames.len() * 2) as f64)
                .sqrt()
        };
        assert!((rms(&result.journey) - rms(&result.crossfade)).abs() < 1e-8);
        assert!(rms(&result.journey) > 0.01);
        assert!(result.routes.len() < MAX_MODES * 2);
        assert!(
            result
                .journey
                .frames
                .iter()
                .zip(result.crossfade.frames.iter())
                .any(|(a, b)| (a[0] - b[0]).abs() > 0.1)
        );
    }

    #[test]
    fn fft_matches_a_direct_transform() {
        let input: Vec<_> = (0..16)
            .map(|i| Complex {
                re: (i as f64 * 0.31).sin(),
                im: 0.,
            })
            .collect();
        let mut actual = input.clone();
        fft(&mut actual);
        for (k, value) in actual.iter().enumerate() {
            let re: f64 = input
                .iter()
                .enumerate()
                .map(|(i, v)| v.re * (-TAU * k as f64 * i as f64 / 16.).cos())
                .sum();
            let im: f64 = input
                .iter()
                .enumerate()
                .map(|(i, v)| v.re * (-TAU * k as f64 * i as f64 / 16.).sin())
                .sum();
            assert!((value.re - re).abs() < 1e-12);
            assert!((value.im - im).abs() < 1e-12);
        }
    }

    #[test]
    fn midpoint_moves_pitch_instead_of_crossfading_two_pitches() {
        let cancel = AtomicBool::new(false);
        let a = analyze(&tone(220., 0.5), 8, &cancel).unwrap();
        let b = analyze(&tone(880., 0.5), 8, &cancel).unwrap();
        assert!((a.modes[0].hz - 220.).abs() < 0.3);
        let settings = Settings {
            seconds: 0.5,
            start: 0.5,
            end: 0.5,
            ..Default::default()
        };
        let result = render(&a, &b, settings, &cancel).unwrap();
        let profile = analyze(&result, 8, &cancel).unwrap();
        assert!(
            (profile.modes[0].hz - 440.).abs() < 0.6,
            "{:?}",
            profile.modes
        );
        assert_eq!(result.frames.len(), 24_000);
        assert_eq!(result.frames[0], [0.; 2]);
        assert_eq!(result.frames.last(), Some(&[0.; 2]));
        assert!(
            result
                .frames
                .iter()
                .flatten()
                .all(|v| v.is_finite() && v.abs() <= 0.850001)
        );
    }

    #[test]
    fn transport_conserves_each_sources_power_and_frequency_order() {
        let a = Profile {
            modes: vec![
                Mode {
                    hz: 100.,
                    power: 0.2,
                    left: 0.5,
                },
                Mode {
                    hz: 400.,
                    power: 0.8,
                    left: 0.5,
                },
            ],
            rms: 0.1,
        };
        let b = Profile {
            modes: vec![
                Mode {
                    hz: 200.,
                    power: 0.6,
                    left: 0.5,
                },
                Mode {
                    hz: 800.,
                    power: 0.4,
                    left: 0.5,
                },
            ],
            rms: 0.1,
        };
        let paths = routes(&a, &b);
        assert_eq!(paths.len(), 3);
        assert!((paths.iter().map(|r| r.power).sum::<f64>() - 1.).abs() < 1e-12);
        for mode in a.modes {
            assert!(
                (paths
                    .iter()
                    .filter(|r| r.from_hz == mode.hz)
                    .map(|r| r.power)
                    .sum::<f64>()
                    - mode.power)
                    .abs()
                    < 1e-12
            );
        }
        for mode in b.modes {
            assert!(
                (paths
                    .iter()
                    .filter(|r| r.to_hz == mode.hz)
                    .map(|r| r.power)
                    .sum::<f64>()
                    - mode.power)
                    .abs()
                    < 1e-12
            );
        }
        assert!(
            paths
                .windows(2)
                .all(|p| p[0].from_hz <= p[1].from_hz && p[0].to_hz <= p[1].to_hz)
        );
    }

    #[test]
    fn stereo_movement_and_determinism_are_preserved() {
        let cancel = AtomicBool::new(false);
        let a = analyze(&tone(330., 1.), 8, &cancel).unwrap();
        let b = analyze(&tone(660., 0.), 8, &cancel).unwrap();
        let settings = Settings {
            seconds: 0.5,
            ..Default::default()
        };
        let result = render(&a, &b, settings, &cancel).unwrap();
        assert_eq!(
            result.frames,
            render(&a, &b, settings, &cancel).unwrap().frames
        );
        let energy = |range: std::ops::Range<usize>, channel: usize| {
            result.frames[range]
                .iter()
                .map(|f| (f[channel] as f64).powi(2))
                .sum::<f64>()
        };
        assert!(energy(1000..4000, 0) > energy(1000..4000, 1) * 10.);
        assert!(energy(20_000..23_000, 1) > energy(20_000..23_000, 0) * 10.);
    }

    #[test]
    fn untrusted_values_silence_dc_limits_and_cancellation_are_rejected() {
        let cancel = AtomicBool::new(false);
        let sample = tone(440., 0.5);
        let profile = analyze(&sample, 8, &cancel).unwrap();
        for value in [f64::NAN, f64::INFINITY, 0., 12.01, -1.] {
            assert!(
                render(
                    &profile,
                    &profile,
                    Settings {
                        seconds: value,
                        ..Default::default()
                    },
                    &cancel
                )
                .is_err()
            );
        }
        for amount in [f64::NAN, -0.1, 1.1] {
            assert!(
                render(
                    &profile,
                    &profile,
                    Settings {
                        start: amount,
                        ..Default::default()
                    },
                    &cancel
                )
                .is_err()
            );
        }
        for value in [0., 0.25, f32::NAN, f32::INFINITY, 17.] {
            let bad = Sample {
                rate: 48_000,
                frames: Arc::new(vec![[value; 2]; 1024]),
            };
            assert!(analyze(&bad, 8, &cancel).is_err());
        }
        assert!(analyze(&sample, 33, &cancel).is_err());
        let high = analyze(&tone(10_000., 0.5), 8, &cancel).unwrap();
        assert!(
            render(
                &profile,
                &high,
                Settings {
                    rate: 8000,
                    ..Default::default()
                },
                &cancel
            )
            .is_err()
        );
        cancel.store(true, Ordering::Relaxed);
        assert!(analyze(&sample, 8, &cancel).is_err());
        assert!(render(&profile, &profile, Settings::default(), &cancel).is_err());
    }
}

use crate::{Error, Result, model::Sample};
use std::{f64::consts::TAU, sync::Arc};

pub fn resample(sample: &Sample, ratio: f64) -> Result<Sample> {
    validate(sample, ratio)?;
    let size = (sample.frames.len() as f64 * ratio).round() as usize;
    if size > 4_000_000 {
        return Err(Error::Limit("Resultado demasiado largo"));
    }
    let frames = (0..size)
        .map(|i| crate::dsp::sinc_sample(&sample.frames, i as f64 / ratio, 1. / ratio))
        .collect();
    Ok(Sample {
        rate: sample.rate,
        frames: Arc::new(frames),
    })
}

pub fn granular(sample: &Sample, time_ratio: f64, semitones: f64) -> Result<Sample> {
    validate(sample, time_ratio)?;
    if !semitones.is_finite() || !(-12. ..=12.).contains(&semitones) {
        return Err(Error::Invalid("Pitch: máximo una octava"));
    }
    if (time_ratio - 1.).abs() < 1e-12 && semitones.abs() < 1e-12 {
        return Ok(sample.clone());
    }
    let size = (sample.frames.len() as f64 * time_ratio).round() as usize;
    if size > 4_000_000 {
        return Err(Error::Limit("Resultado demasiado largo"));
    }
    if size == 0 {
        return Ok(Sample {
            rate: sample.rate,
            frames: Arc::new(Vec::new()),
        });
    }
    let grain = 1024i64;
    let hop = 256usize;
    let pitch = 2f64.powf(semitones / 12.);
    let mut frames = vec![[0f32; 2]; size];
    let mut weights = vec![0f32; size];
    for center in (0..size + grain as usize / 2).step_by(hop) {
        let source = center as f64 / time_ratio;
        for offset in -grain / 2..grain / 2 {
            let target = center as i64 + offset;
            if target < 0 || target >= size as i64 {
                continue;
            }
            let position = source + offset as f64 * pitch;
            let window = (0.5 + 0.5 * (TAU * offset as f64 / grain as f64).cos()) as f32;
            let frame = crate::dsp::lerp_sample(&sample.frames, position);
            let i = target as usize;
            frames[i][0] += frame[0] * window;
            frames[i][1] += frame[1] * window;
            weights[i] += window;
        }
    }
    for (frame, weight) in frames.iter_mut().zip(weights) {
        if weight > 1e-6 {
            frame[0] /= weight;
            frame[1] /= weight;
        }
    }
    Ok(Sample {
        rate: sample.rate,
        frames: Arc::new(frames),
    })
}

fn validate(sample: &Sample, ratio: f64) -> Result<()> {
    if !ratio.is_finite() || !(0.5..=2.).contains(&ratio) {
        return Err(Error::Invalid("Duración: factor entre 0.5 y 2"));
    }
    if sample.frames.len() > 4_000_000 || !(8_000..=192_000).contains(&sample.rate) {
        return Err(Error::Limit("Muestra no válida"));
    }
    if sample
        .frames
        .iter()
        .flatten()
        .any(|x| !x.is_finite() || x.abs() > 16.)
    {
        return Err(Error::Invalid("Audio no válido"));
    }
    Ok(())
}

use crate::{Error, Result, model::Sample};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub enum Edit {
    Reverse,
    Normalize,
    FadeIn,
    FadeOut,
    Silence,
    Gain(f32),
    Trim,
}

pub fn apply(sample: &Sample, start: usize, end: usize, edit: Edit) -> Result<Sample> {
    if start >= end || end > sample.frames.len() {
        return Err(Error::Invalid("Selección de audio no válida"));
    }
    if sample.frames.len() > 4_000_000 {
        return Err(Error::Limit("Audio demasiado grande"));
    }
    if let Edit::Gain(gain) = edit {
        crate::model::finite(gain, 0., 8.)?;
    }
    let mut frames = sample.frames.as_ref().clone();
    let selection = &mut frames[start..end];
    let size = selection.len();
    match edit {
        Edit::Reverse => selection.reverse(),
        Edit::Normalize => {
            let peak = selection
                .iter()
                .flatten()
                .fold(0f32, |a, &b| a.max(b.abs()));
            if peak > 1e-9 {
                for frame in selection {
                    for x in frame {
                        *x = *x / peak * 0.8;
                    }
                }
            }
        }
        Edit::FadeIn | Edit::FadeOut => {
            for (i, frame) in selection.iter_mut().enumerate() {
                let t = if size == 1 {
                    0.
                } else {
                    i as f32 / (size - 1) as f32
                };
                let gain = if matches!(edit, Edit::FadeOut) {
                    1. - t
                } else {
                    t
                };
                for x in frame {
                    *x *= gain;
                }
            }
        }
        Edit::Silence => selection.fill([0.; 2]),
        Edit::Gain(gain) => {
            for frame in selection {
                for x in frame {
                    *x = (*x * gain).clamp(-0.8, 0.8);
                }
            }
        }
        Edit::Trim => frames = selection.to_vec(),
    }
    Ok(Sample {
        rate: sample.rate,
        frames: Arc::new(frames),
    })
}

fn validate(sample: &Sample) -> Result<()> {
    if !(8000..=192000).contains(&sample.rate) || sample.frames.len() > 4_000_000 {
        return Err(Error::Limit("Muestra fuera de los límites"));
    }
    if sample
        .frames
        .iter()
        .flatten()
        .any(|v| !v.is_finite() || v.abs() > 16.)
    {
        return Err(Error::Invalid("Audio no válido"));
    }
    Ok(())
}

pub fn copy(sample: &Sample, start: usize, end: usize) -> Result<Sample> {
    validate(sample)?;
    if start >= end || end > sample.frames.len() {
        return Err(Error::Invalid("Selección de audio no válida"));
    }
    Ok(Sample {
        rate: sample.rate,
        frames: Arc::new(sample.frames[start..end].to_vec()),
    })
}

pub fn splice(
    sample: &Sample,
    start: usize,
    end: usize,
    replacement: Option<&Sample>,
) -> Result<Sample> {
    validate(sample)?;
    if start > end || end > sample.frames.len() {
        return Err(Error::Invalid("Selección de audio no válida"));
    }
    let inserted = if let Some(other) = replacement {
        validate(other)?;
        (other.frames.len() as u64 * sample.rate as u64 + other.rate as u64 / 2) / other.rate as u64
    } else {
        0
    };
    let length = sample.frames.len() as u64 - (end - start) as u64 + inserted;
    if length > 4_000_000 {
        return Err(Error::Limit("Muestra resultante demasiado grande"));
    }
    let mut frames = Vec::with_capacity(length as usize);
    frames.extend_from_slice(&sample.frames[..start]);
    if let Some(other) = replacement {
        if other.rate == sample.rate {
            frames.extend_from_slice(&other.frames);
        } else {
            let ratio = other.rate as f64 / sample.rate as f64;
            for i in 0..inserted {
                frames.push(crate::dsp::sinc_sample(
                    &other.frames,
                    i as f64 * ratio,
                    ratio,
                ));
            }
        }
    }
    frames.extend_from_slice(&sample.frames[end..]);
    Ok(Sample {
        rate: sample.rate,
        frames: Arc::new(frames),
    })
}

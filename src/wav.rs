use crate::{
    Error, Result,
    codec::Reader,
    model::{MAX_FILE, Sample},
};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub enum Depth {
    Pcm16,
    Pcm24,
    Float32,
}

pub fn decode(data: &[u8]) -> Result<Sample> {
    if data.len() > MAX_FILE {
        return Err(Error::Limit("WAV demasiado grande"));
    }
    let mut r = Reader::new(data);
    if r.take(4)? != b"RIFF" {
        return Err(Error::Unsupported("Solo WAV RIFF little-endian"));
    }
    let size = r.u32()? as usize;
    if size.checked_add(8) != Some(data.len()) || r.take(4)? != b"WAVE" {
        return Err(Error::Invalid("Cabecera WAV no válida"));
    }
    let mut format = None;
    let mut audio = None;
    while r.remaining() > 0 {
        let tag = r.take(4)?;
        let len = r.u32()? as usize;
        let bytes = r.take(len)?;
        if len % 2 == 1 {
            r.take(1)?;
        }
        if tag == b"fmt " {
            if format.is_some() {
                return Err(Error::Invalid("Formato WAV duplicado"));
            }
            let mut f = Reader::new(bytes);
            let encoding = f.u16()?;
            let channels = f.u16()?;
            let rate = f.u32()?;
            let byte_rate = f.u32()?;
            let align = f.u16()?;
            let bits = f.u16()?;
            if !(1..=2).contains(&channels)
                || !(8_000..=192_000).contains(&rate)
                || !matches!((encoding, bits), (1, 16) | (1, 24) | (1, 32) | (3, 32))
            {
                return Err(Error::Unsupported(
                    "WAV: PCM 16/24/32 o float32, mono o estéreo",
                ));
            }
            if align as u32 != channels as u32 * (bits as u32 / 8)
                || byte_rate != rate * align as u32
            {
                return Err(Error::Invalid("Alineación WAV incorrecta"));
            }
            format = Some((encoding, channels, rate, align, bits));
        } else if tag == b"data" {
            if audio.is_some() {
                return Err(Error::Invalid("Audio WAV duplicado"));
            }
            audio = Some(bytes);
        }
    }
    let (encoding, channels, rate, align, bits) =
        format.ok_or(Error::Invalid("Falta formato WAV"))?;
    let bytes = audio.ok_or(Error::Invalid("Falta audio WAV"))?;
    if bytes.len() % align as usize != 0 {
        return Err(Error::Invalid("Frame WAV truncado"));
    }
    let count = bytes.len() / align as usize;
    if count > 4_000_000 {
        return Err(Error::Limit("Muestra: máximo cuatro millones de frames"));
    }
    let mut frames = Vec::with_capacity(count);
    let mut r = Reader::new(bytes);
    for _ in 0..count {
        let mut frame = [0.; 2];
        for item in frame.iter_mut().take(channels as usize) {
            *item = match (encoding, bits) {
                (1, 16) => r.u16()? as i16 as f32 / 32768.,
                (1, 24) => {
                    let b = r.take(3)?;
                    let value =
                        ((b[0] as i32) | ((b[1] as i32) << 8) | ((b[2] as i32) << 16)) << 8 >> 8;
                    value as f32 / 8_388_608.
                }
                (1, 32) => r.u32()? as i32 as f32 / 2_147_483_648.,
                (3, 32) => r.f32()?,
                _ => return Err(Error::Unsupported("Codificación WAV no compatible")),
            };
            if !item.is_finite() || item.abs() > 16. {
                return Err(Error::Invalid(
                    "WAV con valores no finitos o fuera de rango",
                ));
            }
        }
        if channels == 1 {
            frame[1] = frame[0];
        }
        frames.push(frame);
    }
    Ok(Sample {
        rate,
        frames: Arc::new(frames),
    })
}

pub fn encode(frames: &[[f32; 2]], rate: u32, depth: Depth, dither: bool) -> Result<Vec<u8>> {
    encode_channels(frames, rate, depth, dither, 2)
}
pub fn encode_channels(
    frames: &[[f32; 2]],
    rate: u32,
    depth: Depth,
    dither: bool,
    channels: u16,
) -> Result<Vec<u8>> {
    if !(1..=2).contains(&channels) {
        return Err(Error::Invalid("Número de canales WAV no válido"));
    }
    if !(8_000..=192_000).contains(&rate) {
        return Err(Error::Invalid("Frecuencia WAV no válida"));
    }
    let (encoding, bits) = match depth {
        Depth::Pcm16 => (1u16, 16u16),
        Depth::Pcm24 => (1, 24),
        Depth::Float32 => (3, 32),
    };
    let align = bits / 8 * channels;
    let len = frames
        .len()
        .checked_mul(align as usize)
        .filter(|n| *n < 128 * 1024 * 1024 - 44)
        .ok_or(Error::Limit("WAV demasiado grande"))?;
    let padding = len % 2;
    let mut w = Vec::with_capacity(len + 44 + padding);
    w.extend(b"RIFF");
    w.extend(((len + padding) as u32 + 36).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(encoding.to_le_bytes());
    w.extend(channels.to_le_bytes());
    w.extend(rate.to_le_bytes());
    w.extend((rate * align as u32).to_le_bytes());
    w.extend(align.to_le_bytes());
    w.extend(bits.to_le_bytes());
    w.extend(b"data");
    w.extend((len as u32).to_le_bytes());
    let mut rng = 0x1234abcd1234abcd;
    for frame in frames {
        if frame.iter().any(|v| !v.is_finite()) {
            return Err(Error::Invalid("No se exporta audio no finito"));
        }
        let mono = [frame[0] * 0.5 + frame[1] * 0.5];
        let source = if channels == 1 { &mono[..] } else { &frame[..] };
        for &sample in source {
            if !sample.is_finite() {
                return Err(Error::Invalid("No se exporta audio no finito"));
            }
            let noise = if dither && encoding == 1 {
                (crate::dsp::noise(&mut rng) + crate::dsp::noise(&mut rng)) * 0.5
            } else {
                0.
            };
            match depth {
                Depth::Pcm16 => w.extend(
                    ((sample as f64 * 32768. + noise)
                        .round()
                        .clamp(-32768., 32767.) as i16)
                        .to_le_bytes(),
                ),
                Depth::Pcm24 => {
                    let v = ((sample as f64 * 8_388_608. + noise)
                        .round()
                        .clamp(-8_388_608., 8_388_607.) as i32)
                        .to_le_bytes();
                    w.extend(&v[..3]);
                }
                Depth::Float32 => w.extend(sample.to_le_bytes()),
            }
        }
    }
    if padding != 0 {
        w.push(0);
    }
    Ok(w)
}

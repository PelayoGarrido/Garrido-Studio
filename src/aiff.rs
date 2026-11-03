use crate::{
    Error, Result,
    codec::Reader,
    model::{MAX_FILE, Sample},
    wav::Depth,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn u16(reader: &mut Reader<'_>) -> Result<u16> {
    Ok(u16::from_be_bytes(
        reader
            .take(2)?
            .try_into()
            .map_err(|_| Error::Invalid("Entero AIFF truncado"))?,
    ))
}
fn u32(reader: &mut Reader<'_>) -> Result<u32> {
    Ok(u32::from_be_bytes(
        reader
            .take(4)?
            .try_into()
            .map_err(|_| Error::Invalid("Entero AIFF truncado"))?,
    ))
}
fn rate(reader: &mut Reader<'_>) -> Result<u32> {
    let exponent = u16(reader)?;
    let mantissa = u64::from_be_bytes(
        reader
            .take(8)?
            .try_into()
            .map_err(|_| Error::Invalid("Frecuencia AIFF truncada"))?,
    );
    if exponent & 0x8000 != 0 || !(16383..=16400).contains(&exponent) || mantissa >> 63 != 1 {
        return Err(Error::Unsupported("Frecuencia AIFF no compatible"));
    }
    let value = mantissa as f64 * 2f64.powi(exponent as i32 - 16383 - 63);
    if !(8000. ..=192000.).contains(&value) || (value - value.round()).abs() > 0.00001 {
        return Err(Error::Unsupported("AIFF: frecuencia entera de 8–192 kHz"));
    }
    Ok(value.round() as u32)
}
#[derive(Clone, Copy)]
enum Encoding {
    Big,
    Little,
    Float32,
    Float64,
}

pub fn decode(data: &[u8]) -> Result<Sample> {
    if data.len() > MAX_FILE {
        return Err(Error::Limit("AIFF demasiado grande"));
    }
    let mut reader = Reader::new(data);
    if reader.take(4)? != b"FORM" || u32(&mut reader)? as usize != data.len().saturating_sub(8) {
        return Err(Error::Invalid("Contenedor AIFF no válido"));
    }
    let compressed = match reader.take(4)? {
        b"AIFF" => false,
        b"AIFC" => true,
        _ => return Err(Error::Unsupported("Solo AIFF o AIFF-C")),
    };
    let mut common = None;
    let mut sound = None;
    let mut version = false;
    let mut chunks = 0;
    while reader.remaining() > 0 {
        chunks += 1;
        if chunks > 1024 {
            return Err(Error::Limit("Demasiados bloques AIFF"));
        }
        let tag = reader.take(4)?;
        let length = u32(&mut reader)? as usize;
        let bytes = reader.take(length)?;
        if !length.is_multiple_of(2) {
            reader.take(1)?;
        }
        let mut chunk = Reader::new(bytes);
        match tag {
            b"COMM" => {
                if common.is_some() {
                    return Err(Error::Invalid("Formato AIFF duplicado"));
                }
                let channels = u16(&mut chunk)?;
                let frames = u32(&mut chunk)? as usize;
                let bits = u16(&mut chunk)?;
                let rate = rate(&mut chunk)?;
                if !(1..=2).contains(&channels) || frames > 4_000_000 {
                    return Err(Error::Limit(
                        "AIFF: mono o estéreo y hasta cuatro millones de frames",
                    ));
                }
                let encoding = if compressed {
                    let encoding = match chunk.take(4)? {
                        b"NONE" | b"twos" => Encoding::Big,
                        b"in24" if bits == 24 => Encoding::Big,
                        b"in32" if bits == 32 => Encoding::Big,
                        b"sowt" => Encoding::Little,
                        b"fl32" | b"FL32" if bits == 32 => Encoding::Float32,
                        b"fl64" | b"FL64" if bits == 64 => Encoding::Float64,
                        _ => return Err(Error::Unsupported("Compresión AIFF-C no compatible")),
                    };
                    let name = chunk.u8()? as usize;
                    chunk.take(name)?;
                    if !(name + 1).is_multiple_of(2) {
                        chunk.take(1)?;
                    }
                    encoding
                } else {
                    Encoding::Big
                };
                if chunk.remaining() != 0
                    || (matches!(encoding, Encoding::Big | Encoding::Little)
                        && !(1..=32).contains(&bits))
                {
                    return Err(Error::Unsupported("Formato de muestra AIFF no compatible"));
                }
                common = Some((channels as usize, frames, bits, rate, encoding));
            }
            b"SSND" => {
                if sound.is_some() {
                    return Err(Error::Invalid("Audio AIFF duplicado"));
                }
                let offset = u32(&mut chunk)? as usize;
                let block = u32(&mut chunk)? as usize;
                chunk.take(offset)?;
                sound = Some((chunk.take(chunk.remaining())?, block));
            }
            b"FVER" if compressed => {
                if version || length != 4 || u32(&mut chunk)? != 0xA2805140 {
                    return Err(Error::Unsupported("Versión AIFF-C no compatible"));
                }
                version = true;
            }
            _ => {}
        }
    }
    if compressed && !version {
        return Err(Error::Invalid("Falta la versión AIFF-C"));
    }
    let (channels, count, bits, rate, encoding) =
        common.ok_or(Error::Invalid("Falta el formato AIFF"))?;
    let width = (bits as usize).div_ceil(8);
    let length = count
        .checked_mul(channels * width)
        .ok_or(Error::Limit("AIFF demasiado grande"))?;
    let (bytes, block) = match sound {
        Some(sound) => sound,
        None if count == 0 => (&[][..], 0),
        None => return Err(Error::Invalid("Falta audio AIFF")),
    };
    if bytes.len() < length
        || (bytes.len() > length && (block == 0 || bytes.len() - length >= block))
    {
        return Err(Error::Invalid("Longitud de audio AIFF no válida"));
    }
    let mut reader = Reader::new(&bytes[..length]);
    let mut frames = Vec::with_capacity(count);
    for _ in 0..count {
        let mut frame = [0.; 2];
        for value in frame.iter_mut().take(channels) {
            let bytes = reader.take(width)?;
            *value = match encoding {
                Encoding::Float32 => f32::from_be_bytes(
                    bytes
                        .try_into()
                        .map_err(|_| Error::Invalid("Float AIFF truncado"))?,
                ),
                Encoding::Float64 => f64::from_be_bytes(
                    bytes
                        .try_into()
                        .map_err(|_| Error::Invalid("Float AIFF truncado"))?,
                ) as f32,
                Encoding::Big | Encoding::Little => {
                    let mut raw = 0u32;
                    if matches!(encoding, Encoding::Big) {
                        for byte in bytes {
                            raw = (raw << 8) | *byte as u32;
                        }
                    } else {
                        for byte in bytes.iter().rev() {
                            raw = (raw << 8) | *byte as u32;
                        }
                    }
                    let padding = width as u16 * 8 - bits;
                    if raw & ((1 << padding) - 1) != 0 {
                        return Err(Error::Invalid("Relleno PCM AIFF no válido"));
                    }
                    ((raw << (32 - width * 8)) as i32 as f64 / 2147483648.) as f32
                }
            };
            if !value.is_finite() || value.abs() > 16. {
                return Err(Error::Invalid("Audio AIFF no finito o fuera de rango"));
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

pub fn encode(
    frames: &[[f32; 2]],
    rate: u32,
    depth: Depth,
    channels: u16,
    dither: bool,
) -> Result<Vec<u8>> {
    encode_cancel(
        frames,
        rate,
        depth,
        channels,
        dither,
        &AtomicBool::new(false),
    )
}
pub fn encode_cancel(
    frames: &[[f32; 2]],
    rate: u32,
    depth: Depth,
    channels: u16,
    dither: bool,
    cancel: &AtomicBool,
) -> Result<Vec<u8>> {
    if cancel.load(Ordering::Relaxed) {
        return Err(Error::Invalid("Exportación AIFF cancelada"));
    }
    if !(8000..=192000).contains(&rate) || !(1..=2).contains(&channels) {
        return Err(Error::Invalid("Frecuencia o canales AIFF no válidos"));
    }
    let bits: u16 = match depth {
        Depth::Pcm16 => 16,
        Depth::Pcm24 => 24,
        Depth::Float32 => return Err(Error::Unsupported("Exporta AIFF con PCM16 o PCM24")),
    };
    let length = frames
        .len()
        .checked_mul(channels as usize * bits as usize / 8)
        .filter(|size| *size < 128 * 1024 * 1024 - 55)
        .ok_or(Error::Limit("AIFF demasiado grande"))?;
    let padding = length % 2;
    let mut bytes = Vec::with_capacity(54 + length + padding);
    bytes.extend(b"FORM");
    bytes.extend((46 + length as u32 + padding as u32).to_be_bytes());
    bytes.extend(b"AIFFCOMM");
    bytes.extend(18u32.to_be_bytes());
    bytes.extend(channels.to_be_bytes());
    bytes.extend((frames.len() as u32).to_be_bytes());
    bytes.extend(bits.to_be_bytes());
    let power = 31 - rate.leading_zeros();
    bytes.extend((16383u16 + power as u16).to_be_bytes());
    bytes.extend(((rate as u64) << (63 - power)).to_be_bytes());
    bytes.extend(b"SSND");
    bytes.extend((8 + length as u32).to_be_bytes());
    bytes.extend([0; 8]);
    let mut rng = 0x1234abcd1234abcd;
    for (index, frame) in frames.iter().enumerate() {
        if index % 1024 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(Error::Invalid("Exportación AIFF cancelada"));
        }
        if frame.iter().any(|value| !value.is_finite()) {
            return Err(Error::Invalid("No se exporta audio no finito"));
        }
        let mono = [frame[0] * 0.5 + frame[1] * 0.5];
        let source = if channels == 1 { &mono[..] } else { &frame[..] };
        for sample in source {
            let noise = if dither {
                (crate::dsp::noise(&mut rng) + crate::dsp::noise(&mut rng)) * 0.5
            } else {
                0.
            };
            let scale = (1u32 << (bits - 1)) as f64;
            let raw = (*sample as f64 * scale + noise)
                .round()
                .clamp(-scale, scale - 1.) as i32;
            if bits == 16 {
                bytes.extend((raw as i16).to_be_bytes());
            } else {
                bytes.extend(&raw.to_be_bytes()[1..]);
            }
        }
    }
    if padding != 0 {
        bytes.push(0);
    }
    Ok(bytes)
}

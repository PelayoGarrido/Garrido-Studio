use crate::{
    Error, Result,
    codec::Reader,
    model::{MAX_FILE, Sample},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
mod digest;

struct Bits<'a> {
    data: &'a [u8],
    at: usize,
}
impl<'a> Bits<'a> {
    fn read(&mut self, n: u8) -> Result<u64> {
        if n > 64
            || self
                .at
                .checked_add(n as usize)
                .is_none_or(|end| end > self.data.len() * 8)
        {
            return Err(Error::Invalid("FLAC truncado"));
        }
        let mut value = 0;
        for _ in 0..n {
            value = (value << 1) | ((self.data[self.at / 8] >> (7 - self.at % 8)) & 1) as u64;
            self.at += 1;
        }
        Ok(value)
    }
    fn signed(&mut self, n: u8) -> Result<i64> {
        if n == 0 {
            return Ok(0);
        }
        if n > 63 {
            return Err(Error::Invalid("Precisión FLAC no válida"));
        }
        let v = self.read(n)?;
        Ok(((v << (64 - n)) as i64) >> (64 - n))
    }
    fn unary(&mut self, limit: u32) -> Result<u32> {
        for count in 0..=limit {
            if self.read(1)? == 1 {
                return Ok(count);
            }
        }
        Err(Error::Limit("Código Rice supera el presupuesto"))
    }
    fn align(&mut self) -> Result<()> {
        while !self.at.is_multiple_of(8) {
            if self.read(1)? != 0 {
                return Err(Error::Invalid("Padding FLAC no válido"));
            }
        }
        Ok(())
    }
    fn number(&mut self) -> Result<u64> {
        let first = self.read(8)? as u8;
        if first < 128 {
            return Ok(first as u64);
        }
        let n = first.leading_ones() as u8;
        if !(2..=7).contains(&n) {
            return Err(Error::Invalid("Número FLAC no válido"));
        }
        let mut value = (first & ((1 << (7 - n)) - 1)) as u64;
        for _ in 1..n {
            let b = self.read(8)?;
            if b & 0xc0 != 0x80 {
                return Err(Error::Invalid("Número FLAC no válido"));
            }
            value = (value << 6) | (b & 63);
        }
        let min = [0, 0, 128, 2048, 65536, 1 << 21, 1 << 26, 1 << 31][n as usize];
        if value < min {
            return Err(Error::Invalid("Número FLAC no canónico"));
        }
        Ok(value)
    }
}
struct Writer {
    data: Vec<u8>,
    at: u8,
}
impl Writer {
    fn bits(&mut self, value: u64, n: u8) {
        for shift in (0..n).rev() {
            if self.at == 0 {
                self.data.push(0);
            }
            let last = self.data.len() - 1;
            self.data[last] |= (((value >> shift) & 1) as u8) << (7 - self.at);
            self.at = (self.at + 1) % 8;
        }
    }
    fn align(&mut self) {
        if self.at != 0 {
            self.bits(0, 8 - self.at);
        }
    }
}
const fn crc8_table() -> [u8; 256] {
    let mut out = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut v = i as u8;
        let mut j = 0;
        while j < 8 {
            v = if v & 128 != 0 { (v << 1) ^ 7 } else { v << 1 };
            j += 1;
        }
        out[i] = v;
        i += 1;
    }
    out
}
const fn crc16_table() -> [u16; 256] {
    let mut out = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut v = (i as u16) << 8;
        let mut j = 0;
        while j < 8 {
            v = if v & 0x8000 != 0 {
                (v << 1) ^ 0x8005
            } else {
                v << 1
            };
            j += 1;
        }
        out[i] = v;
        i += 1;
    }
    out
}
fn crc8(data: &[u8]) -> u8 {
    const TABLE: [u8; 256] = crc8_table();
    data.iter().fold(0, |v, b| TABLE[(v ^ b) as usize])
}
fn crc16(data: &[u8]) -> u16 {
    const TABLE: [u16; 256] = crc16_table();
    data.iter()
        .fold(0, |v, b| (v << 8) ^ TABLE[((v >> 8) as u8 ^ b) as usize])
}
fn prediction(samples: &[i64], at: usize, order: usize) -> i64 {
    match order {
        0 => 0,
        1 => samples[at - 1],
        2 => 2 * samples[at - 1] - samples[at - 2],
        3 => 3 * samples[at - 1] - 3 * samples[at - 2] + samples[at - 3],
        _ => 4 * samples[at - 1] - 6 * samples[at - 2] + 4 * samples[at - 3] - samples[at - 4],
    }
}
fn fold(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}
fn residual(bits: &mut Bits<'_>, samples: &mut [i64], order: usize) -> Result<()> {
    let method = bits.read(2)?;
    if method > 1 {
        return Err(Error::Unsupported("Método residual FLAC reservado"));
    }
    let partition = bits.read(4)? as usize;
    let parts = 1usize << partition;
    if !samples.len().is_multiple_of(parts) || samples.len() / parts <= order {
        return Err(Error::Invalid("Partición FLAC no válida"));
    }
    let width = if method == 0 { 4 } else { 5 };
    let escape = (1 << width) - 1;
    for part in 0..parts {
        let k = bits.read(width)?;
        let raw = if k == escape {
            Some(bits.read(5)? as u8)
        } else {
            None
        };
        let start = (part * samples.len() / parts).max(order);
        let end = (part + 1) * samples.len() / parts;
        for sample in &mut samples[start..end] {
            *sample = if let Some(n) = raw {
                bits.signed(n)?
            } else {
                let q = bits.unary(1 << 20)? as u64;
                let value = (q << k) | bits.read(k as u8)?;
                if value >= u32::MAX as u64 {
                    return Err(Error::Invalid("Residual FLAC fuera de rango"));
                }
                ((value >> 1) as i64) ^ -((value & 1) as i64)
            };
        }
    }
    Ok(())
}
fn subframe(bits: &mut Bits<'_>, count: usize, precision: u8) -> Result<Vec<i64>> {
    if bits.read(1)? != 0 {
        return Err(Error::Invalid("Cabecera subframe FLAC no válida"));
    }
    let kind = bits.read(6)? as usize;
    let wasted = if bits.read(1)? != 0 {
        bits.unary(precision as u32)? + 1
    } else {
        0
    };
    if wasted >= precision as u32 {
        return Err(Error::Invalid("Bits desperdiciados FLAC no válidos"));
    }
    let depth = precision - wasted as u8;
    let mut out = vec![0i64; count];
    match kind {
        0 => {
            let v = bits.signed(depth)?;
            out.fill(v);
        }
        1 => {
            for v in &mut out {
                *v = bits.signed(depth)?;
            }
        }
        8..=12 | 32..=63 => {
            let order = if kind < 32 { kind - 8 } else { kind - 31 };
            if order >= count {
                return Err(Error::Invalid("Orden predictor FLAC no válido"));
            }
            for v in &mut out[..order] {
                *v = bits.signed(depth)?;
            }
            let mut coefficients = [0i64; 32];
            let mut shift = 0;
            if kind >= 32 {
                let n = bits.read(4)? as u8 + 1;
                if n == 16 {
                    return Err(Error::Invalid("Precisión LPC reservada"));
                }
                shift = bits.signed(5)?;
                if shift < 0 {
                    return Err(Error::Invalid("Desplazamiento LPC negativo"));
                }
                for c in &mut coefficients[..order] {
                    *c = bits.signed(n)?;
                }
            }
            residual(bits, &mut out, order)?;
            let min = -(1i64 << (depth - 1));
            let max = (1i64 << (depth - 1)) - 1;
            for i in order..count {
                let predicted = if kind < 32 {
                    prediction(&out, i, order)
                } else {
                    let mut sum = 0i64;
                    for (j, c) in coefficients[..order].iter().enumerate() {
                        sum = sum
                            .checked_add(
                                c.checked_mul(out[i - j - 1])
                                    .ok_or(Error::Invalid("LPC desbordado"))?,
                            )
                            .ok_or(Error::Invalid("LPC desbordado"))?;
                    }
                    sum >> shift
                };
                out[i] = out[i]
                    .checked_add(predicted)
                    .filter(|v| (min..=max).contains(v))
                    .ok_or(Error::Invalid("Muestra FLAC fuera de rango"))?;
            }
        }
        _ => return Err(Error::Unsupported("Subframe FLAC reservado")),
    }
    for v in &mut out {
        *v <<= wasted;
    }
    Ok(out)
}

pub fn decode(data: &[u8]) -> Result<Sample> {
    if data.len() > MAX_FILE {
        return Err(Error::Limit("FLAC: máximo 64 MiB"));
    }
    let mut reader = Reader::new(data);
    if reader.take(4)? != b"fLaC" {
        return Err(Error::Invalid("Falta firma FLAC"));
    }
    let mut stream = None;
    let mut expected_digest = [0u8; 16];
    let mut blocks = 0;
    loop {
        blocks += 1;
        if blocks > 1024 {
            return Err(Error::Limit("Demasiados metadatos FLAC"));
        }
        let flag = reader.u8()?;
        let length = reader.take(3)?;
        let length =
            ((length[0] as usize) << 16) | ((length[1] as usize) << 8) | length[2] as usize;
        let bytes = reader.take(length)?;
        if flag & 127 == 0 {
            if blocks != 1 || bytes.len() != 34 {
                return Err(Error::Invalid("Streaminfo FLAC no válido"));
            }
            let mut bits = Bits { data: bytes, at: 0 };
            let min = bits.read(16)? as usize;
            let max = bits.read(16)? as usize;
            bits.read(48)?;
            let rate = bits.read(20)? as u32;
            let channels = bits.read(3)? as usize + 1;
            let depth = bits.read(5)? as u8 + 1;
            let total = bits.read(36)? as usize;
            expected_digest.copy_from_slice(&bytes[18..34]);
            if min < 16
                || max < min
                || channels > 2
                || !(4..=32).contains(&depth)
                || !(8000..=192000).contains(&rate)
                || total > 4_000_000
            {
                return Err(Error::Unsupported(
                    "FLAC fuera de los límites de audio admitidos",
                ));
            }
            stream = Some((min, max, rate, channels, depth, total));
        } else if flag & 127 == 127 || blocks == 1 {
            return Err(Error::Invalid("Metadato FLAC no válido"));
        }
        if flag & 128 != 0 {
            break;
        }
    }
    let (minimum, maximum, rate, channels, depth, total) =
        stream.ok_or(Error::Invalid("Falta streaminfo FLAC"))?;
    let mut audio = Vec::with_capacity(total.min(4_000_000));
    let mut digest = digest::Digest::new();
    let sample_bytes = depth.div_ceil(8) as usize;
    let mut frame = 0u64;
    let mut strategy = None;
    while reader.remaining() > 0 {
        if frame >= 262144 {
            return Err(Error::Limit("Demasiados frames FLAC"));
        }
        let start = reader.position;
        let mut bits = Bits {
            data: &data[start..],
            at: 0,
        };
        if bits.read(15)? != 0x7ffc {
            return Err(Error::Invalid("Sincronización FLAC no válida"));
        }
        let variable = bits.read(1)?;
        if strategy.is_some_and(|v| v != variable) {
            return Err(Error::Invalid("Estrategia FLAC cambiante"));
        }
        strategy = Some(variable);
        let size_code = bits.read(4)?;
        let rate_code = bits.read(4)?;
        let assignment = bits.read(4)?;
        let precision_code = bits.read(3)?;
        if bits.read(1)? != 0 {
            return Err(Error::Invalid("Bit FLAC reservado"));
        }
        let number = bits.number()?;
        if number
            != if variable == 1 {
                audio.len() as u64
            } else {
                frame
            }
        {
            return Err(Error::Invalid("Secuencia de frames FLAC no válida"));
        }
        let count = match size_code {
            1 => 192,
            2..=5 => 576usize << (size_code - 2),
            6 => bits.read(8)? as usize + 1,
            7 => bits.read(16)? as usize + 1,
            8..=15 => 1usize << size_code,
            _ => return Err(Error::Invalid("Bloque FLAC reservado")),
        };
        if count > maximum || count > 65535 || audio.len() + count > 4_000_000 {
            return Err(Error::Limit("Audio FLAC demasiado grande"));
        }
        let frame_rate = match rate_code {
            0 => rate,
            1 => 88200,
            2 => 176400,
            3 => 192000,
            4 => 8000,
            5 => 16000,
            6 => 22050,
            7 => 24000,
            8 => 32000,
            9 => 44100,
            10 => 48000,
            11 => 96000,
            12 => bits.read(8)? as u32 * 1000,
            13 => bits.read(16)? as u32,
            14 => bits.read(16)? as u32 * 10,
            _ => return Err(Error::Invalid("Frecuencia FLAC reservada")),
        };
        let frame_depth = match precision_code {
            0 => depth,
            1 => 8,
            2 => 12,
            4 => 16,
            5 => 20,
            6 => 24,
            7 => 32,
            _ => return Err(Error::Invalid("Precisión FLAC reservada")),
        };
        if frame_rate != rate
            || frame_depth != depth
            || !matches!((channels, assignment), (1, 0) | (2, 1 | 8 | 9 | 10))
        {
            return Err(Error::Unsupported(
                "Cambio de formato o canales FLAC no admitidos",
            ));
        }
        let header = bits.at / 8;
        if bits.read(8)? as u8 != crc8(&data[start..start + header]) {
            return Err(Error::Invalid("CRC8 FLAC incorrecto"));
        }
        let mut decoded = Vec::with_capacity(channels);
        for channel in 0..channels {
            let side = (assignment == 8 && channel == 1)
                || (assignment == 9 && channel == 0)
                || (assignment == 10 && channel == 1);
            decoded.push(subframe(&mut bits, count, depth + u8::from(side))?);
        }
        bits.align()?;
        let bytes = bits.at / 8;
        let expected = bits.read(16)? as u16;
        if crc16(&data[start..start + bytes]) != expected {
            return Err(Error::Invalid("CRC16 FLAC incorrecto"));
        }
        reader.take(bits.at / 8)?;
        if reader.remaining() > 0 && (count < minimum || (variable == 0 && count != maximum)) {
            return Err(Error::Invalid("Tamaño de bloque FLAC inconsistente"));
        }
        let limit = 1i64 << (depth - 1);
        let right = decoded.get(1).unwrap_or(&decoded[0]);
        for (&a, &b) in decoded[0].iter().zip(right) {
            let (left, right) = match assignment {
                8 => (a, a - b),
                9 => (a + b, b),
                10 => {
                    let mid = (a << 1) | (b & 1);
                    ((mid + b) >> 1, (mid - b) >> 1)
                }
                _ => (a, b),
            };
            if !(-limit..limit).contains(&left) || !(-limit..limit).contains(&right) {
                return Err(Error::Invalid("Éstereo FLAC fuera de rango"));
            }
            digest.update(&left.to_le_bytes()[..sample_bytes]);
            if channels == 2 {
                digest.update(&right.to_le_bytes()[..sample_bytes]);
            }
            audio.push([
                (left as f64 / limit as f64) as f32,
                (right as f64 / limit as f64) as f32,
            ]);
        }
        frame += 1;
    }
    if frame == 0 || (total != 0 && audio.len() != total) {
        return Err(Error::Invalid("Número de muestras FLAC incorrecto"));
    }
    if expected_digest != [0; 16] && digest.finish() != expected_digest {
        return Err(Error::Invalid(
            "El audio FLAC no coincide con su checksum MD5",
        ));
    }
    Ok(Sample {
        rate,
        frames: Arc::new(audio),
    })
}

fn encode_channel(writer: &mut Writer, samples: &[i64], depth: u8) {
    if samples.iter().all(|v| *v == samples[0]) {
        writer.bits(0, 8);
        writer.bits(samples[0] as u64, depth);
        return;
    }
    let mut best = (samples.len() as u64 * depth as u64, usize::MAX, 0u8);
    for order in 0..=4.min(samples.len() - 1) {
        let residuals: Vec<_> = samples
            .iter()
            .enumerate()
            .skip(order)
            .map(|(i, v)| fold(*v - prediction(samples, i, order)))
            .collect();
        let mean = residuals.iter().sum::<u64>() / residuals.len() as u64;
        let base = (63 - mean.max(1).leading_zeros()).min(14) as u8;
        for k in base.saturating_sub(1)..=(base + 1).min(14) {
            let cost = order as u64 * depth as u64
                + 10
                + residuals
                    .iter()
                    .map(|v| (v >> k) + 1 + k as u64)
                    .sum::<u64>();
            if cost < best.0 {
                best = (cost, order, k);
            }
        }
    }
    if best.1 == usize::MAX {
        writer.bits(2, 8);
        for sample in samples {
            writer.bits(*sample as u64, depth);
        }
        return;
    }
    let (_, order, k) = best;
    writer.bits(((8 + order) << 1) as u64, 8);
    for sample in &samples[..order] {
        writer.bits(*sample as u64, depth);
    }
    writer.bits(0, 6);
    writer.bits(k as u64, 4);
    for (i, sample) in samples.iter().enumerate().skip(order) {
        let value = fold(*sample - prediction(samples, i, order));
        for _ in 0..value >> k {
            writer.bits(0, 1);
        }
        writer.bits(1, 1);
        writer.bits(value, k);
    }
}
fn number(out: &mut Vec<u8>, n: u64) {
    if n < 128 {
        out.push(n as u8);
        return;
    }
    let count = if n < 2048 {
        2
    } else if n < 65536 {
        3
    } else if n < 1 << 21 {
        4
    } else if n < 1 << 26 {
        5
    } else {
        6
    };
    out.push(((0xffu16 << (8 - count)) as u8) | ((n >> (6 * (count - 1))) as u8));
    for i in (0..count - 1).rev() {
        out.push(0x80 | ((n >> (6 * i)) & 63) as u8);
    }
}
pub fn encode(audio: &[[f32; 2]], rate: u32, depth: u8, channels: u16) -> Result<Vec<u8>> {
    encode_cancel(audio, rate, depth, channels, &AtomicBool::new(false))
}
pub fn encode_cancel(
    audio: &[[f32; 2]],
    rate: u32,
    depth: u8,
    channels: u16,
    cancel: &AtomicBool,
) -> Result<Vec<u8>> {
    if audio.is_empty()
        || audio.len() > 12_000_000
        || !(8000..=192000).contains(&rate)
        || ![16, 24].contains(&depth)
        || !(1..=2).contains(&channels)
    {
        return Err(Error::Invalid("Ajustes FLAC no válidos"));
    }
    if audio.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error::Invalid("No se exporta audio no finito"));
    }
    let mut out = b"fLaC".to_vec();
    out.extend([0x80, 0, 0, 34]);
    out.extend(4096u16.to_be_bytes());
    out.extend(4096u16.to_be_bytes());
    out.extend([0; 6]);
    let packed = ((rate as u64) << 44)
        | (((channels - 1) as u64) << 41)
        | (((depth - 1) as u64) << 36)
        | audio.len() as u64;
    out.extend(packed.to_be_bytes());
    out.extend([0; 16]);
    let scale = (1i64 << (depth - 1)) as f64;
    let mut digest = digest::Digest::new();
    for (index, block) in audio.chunks(4096).enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Invalid("Exportación FLAC cancelada"));
        }
        let start = out.len();
        let code = match rate {
            88200 => 1,
            176400 => 2,
            192000 => 3,
            8000 => 4,
            16000 => 5,
            22050 => 6,
            24000 => 7,
            32000 => 8,
            44100 => 9,
            48000 => 10,
            96000 => 11,
            _ if rate <= 65535 => 13,
            _ if rate.is_multiple_of(10) => 14,
            _ => 0,
        };
        out.extend([
            0xff,
            0xf8,
            0x70 | code,
            ((channels as u8 - 1) << 4) | if depth == 16 { 8 } else { 12 },
        ]);
        number(&mut out, index as u64);
        out.extend(((block.len() - 1) as u16).to_be_bytes());
        if code == 13 {
            out.extend((rate as u16).to_be_bytes());
        } else if code == 14 {
            out.extend(((rate / 10) as u16).to_be_bytes());
        }
        out.push(crc8(&out[start..]));
        let mut writer = Writer {
            data: Vec::new(),
            at: 0,
        };
        let mut quantized = Vec::with_capacity(channels as usize);
        for channel in 0..channels as usize {
            let samples: Vec<_> = block
                .iter()
                .map(|f| {
                    let v = if channels == 1 {
                        f[0] as f64 * 0.5 + f[1] as f64 * 0.5
                    } else {
                        f[channel] as f64
                    };
                    (v * scale).round().clamp(-scale, scale - 1.) as i64
                })
                .collect();
            encode_channel(&mut writer, &samples, depth);
            quantized.push(samples);
        }
        for index in 0..block.len() {
            for channel in &quantized {
                digest.update(&channel[index].to_le_bytes()[..(depth / 8) as usize]);
            }
        }
        writer.align();
        out.extend(writer.data);
        let crc = crc16(&out[start..]);
        out.extend(crc.to_be_bytes());
        if out.len() > 128 * 1024 * 1024 {
            return Err(Error::Limit("FLAC de salida demasiado grande"));
        }
    }
    out[26..42].copy_from_slice(&digest.finish());
    Ok(out)
}

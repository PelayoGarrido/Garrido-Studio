use crate::{Error, Result, codec::Reader, model::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    sync::Arc,
};

mod playlist;

const LIMIT: usize = 32 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct Event {
    pub id: u8,
    pub bytes: Range<usize>,
    pub payload: Range<usize>,
}
#[derive(Clone)]
pub struct Document {
    bytes: Arc<Vec<u8>>,
    pub events: Vec<Event>,
    data: Range<usize>,
    pub format: u16,
    pub channels: u16,
    pub ppq: u16,
}
pub struct Import {
    pub project: Project,
    pub warnings: Vec<String>,
    bindings: Vec<Binding>,
    channel_ids: Vec<u16>,
}
struct Binding {
    source_pattern: u16,
    channel: u16,
    pattern: usize,
}

fn varint(r: &mut Reader<'_>) -> Result<usize> {
    let mut n = 0u32;
    for i in 0..5 {
        let b = r.u8()?;
        if i == 4 && b > 15 {
            return Err(Error::Invalid("Longitud FLP desbordada"));
        }
        n |= ((b & 127) as u32) << (i * 7);
        if b & 128 == 0 {
            return Ok(n as usize);
        }
    }
    Err(Error::Invalid("Varint FLP no válido"))
}
fn write_varint(out: &mut Vec<u8>, mut n: usize) {
    loop {
        let rest = n >> 7;
        out.push((n & 127) as u8 | if rest > 0 { 128 } else { 0 });
        if rest == 0 {
            break;
        }
        n = rest;
    }
}
fn u16_at(data: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([data[at], data[at + 1]])
}
fn u32_at(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}
fn decoded_text(data: &[u8]) -> String {
    if data.len() >= 2 && data.len().is_multiple_of(2) && data.ends_with(&[0, 0]) {
        char::decode_utf16(
            data.as_chunks::<2>()
                .0
                .iter()
                .take(256)
                .map(|b| u16::from_le_bytes([b[0], b[1]])),
        )
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .filter(|c| !c.is_control())
        .take(64)
        .collect()
    } else {
        String::from_utf8_lossy(&data[..data.len().min(256)])
            .chars()
            .filter(|c| !c.is_control())
            .take(64)
            .collect()
    }
}

impl Document {
    pub fn parse(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() > LIMIT {
            return Err(Error::Limit("FLP: máximo 32 MiB"));
        }
        let mut r = Reader::new(&bytes);
        if r.take(4)? != b"FLhd" {
            return Err(Error::Invalid("Falta cabecera FLhd"));
        }
        let size = r.u32()? as usize;
        if !(6..=64).contains(&size) {
            return Err(Error::Invalid("Cabecera FLP no válida"));
        }
        let header = r.take(size)?;
        let format = u16_at(header, 0);
        let channels = u16_at(header, 2);
        let ppq = u16_at(header, 4);
        if ppq == 0 || ppq > 9600 || channels > 4096 {
            return Err(Error::Invalid("Tiempo o canales FLP fuera de rango"));
        }
        let mut data = None;
        while r.remaining() > 0 {
            let tag = r.take(4)?;
            let size = r.u32()? as usize;
            let start = r.position;
            r.take(size)?;
            if tag == b"FLdt" {
                if data.is_some() {
                    return Err(Error::Invalid("FLdt duplicado"));
                }
                data = Some(start..start + size);
            }
        }
        let data = data.ok_or(Error::Invalid("Falta FLdt"))?;
        let mut r = Reader::new(&bytes[data.clone()]);
        let mut events = Vec::new();
        while r.remaining() > 0 {
            if events.len() >= 262_144 {
                return Err(Error::Limit("Demasiados eventos FLP"));
            }
            let start = r.position;
            let id = r.u8()?;
            let size = match id {
                0..=63 => 1,
                64..=127 => 2,
                128..=191 => 4,
                _ => varint(&mut r)?,
            };
            let payload = r.position;
            r.take(size)?;
            events.push(Event {
                id,
                bytes: data.start + start..data.start + r.position,
                payload: data.start + payload..data.start + payload + size,
            });
        }
        Ok(Self {
            bytes: Arc::new(bytes),
            events,
            data,
            format,
            channels,
            ppq,
        })
    }
    pub fn original(&self) -> &[u8] {
        &self.bytes
    }
    pub fn value(&self, event: &Event) -> &[u8] {
        &self.bytes[event.payload.clone()]
    }
    pub fn rewrite(&self, replacements: &BTreeMap<usize, Vec<u8>>) -> Result<Vec<u8>> {
        let mut body = Vec::with_capacity(self.data.len());
        for (i, e) in self.events.iter().enumerate() {
            if let Some(value) = replacements.get(&i) {
                if value.len() > LIMIT.saturating_sub(body.len() + 6) {
                    return Err(Error::Limit("FLP demasiado grande"));
                }
                let expected = match e.id {
                    0..=63 => Some(1),
                    64..=127 => Some(2),
                    128..=191 => Some(4),
                    _ => None,
                };
                if expected.is_some_and(|n| n != value.len()) {
                    return Err(Error::Invalid("Tamaño fijo FLP incorrecto"));
                }
                body.push(e.id);
                if e.id >= 192 {
                    write_varint(&mut body, value.len());
                }
                body.extend(value);
            } else {
                body.extend(&self.bytes[e.bytes.clone()]);
            }
            if body.len() > LIMIT {
                return Err(Error::Limit("FLP demasiado grande"));
            }
        }
        if replacements.keys().any(|&i| i >= self.events.len()) {
            return Err(Error::Invalid("Evento FLP inexistente"));
        }
        let mut out = self.bytes[..self.data.start - 4].to_vec();
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(body);
        out.extend(&self.bytes[self.data.end..]);
        if out.len() > LIMIT {
            return Err(Error::Limit("FLP demasiado grande"));
        }
        Ok(out)
    }
    pub fn with_tempo(&self, bpm: f64) -> Result<Vec<u8>> {
        if !bpm.is_finite() || !(20. ..=300.).contains(&bpm) {
            return Err(Error::Invalid("Tempo no válido"));
        }
        let mut replacements = BTreeMap::new();
        for (i, e) in self.events.iter().enumerate() {
            match e.id {
                156 => {
                    replacements.insert(i, ((bpm * 1000.).round() as u32).to_le_bytes().to_vec());
                }
                66 => {
                    replacements.insert(i, (bpm.round() as u16).to_le_bytes().to_vec());
                }
                _ => {}
            }
        }
        if replacements.is_empty() {
            return Err(Error::Unsupported("Este FLP no contiene un tempo editable"));
        }
        self.rewrite(&replacements)
    }
    pub fn import(&self) -> Result<Import> {
        if self.format != 0 {
            return Err(Error::Unsupported(
                "Solo proyectos FLP; no presets ni scores",
            ));
        }
        let mut project = Project::empty();
        project.title = "FLP importado".into();
        let mut pattern = 1u16;
        let mut channel = 0u16;
        let mut names = BTreeMap::new();
        let mut pattern_names = BTreeMap::new();
        let mut declared = BTreeSet::new();
        let mut explicit_lengths = BTreeMap::new();
        let mut warnings = Vec::new();
        let playlist = self.playlist(&mut warnings)?;
        let mut groups: BTreeMap<(u16, u16), Vec<Note>> = BTreeMap::new();
        let mut total = 0;
        let mut unsupported = BTreeSet::new();
        let mut special_notes = 0;
        let scale = |n: u32| -> Result<u32> {
            let tick = (n as u64 * PPQ as u64 + self.ppq as u64 / 2) / self.ppq as u64;
            if tick > MAX_TICKS as u64 {
                Err(Error::Limit("FLP supera 256 compases"))
            } else {
                Ok(tick as u32)
            }
        };
        for e in &self.events {
            let d = self.value(e);
            match e.id {
                64 => channel = u16_at(d, 0),
                65 => {
                    pattern = u16_at(d, 0);
                    declared.insert(pattern);
                }
                164 => {
                    let length = u32_at(d, 0);
                    if length > 0 {
                        explicit_lengths.insert(pattern, scale(length)?);
                    }
                }
                66 => project.bpm = u16_at(d, 0) as f64,
                156 => project.bpm = u32_at(d, 0) as f64 / 1000.,
                194 => project.title = decoded_text(d),
                192 | 203 => {
                    names.insert(channel, decoded_text(d));
                }
                193 => {
                    pattern_names.insert(pattern, decoded_text(d));
                }
                224 => {
                    if !d.len().is_multiple_of(24) {
                        return Err(Error::Unsupported(
                            "FLP: estructura de notas no compatible (24 bytes)",
                        ));
                    }
                    for n in d.as_chunks::<24>().0 {
                        total += 1;
                        if total > MAX_NOTES {
                            return Err(Error::Limit("Demasiadas notas FLP"));
                        }
                        let tick = scale(u32_at(n, 0))?;
                        let raw_length = u32_at(n, 8);
                        let length = if raw_length == 0 {
                            PPQ / 4
                        } else {
                            scale(raw_length)?.max(1)
                        };
                        let key = u16_at(n, 12);
                        if key > 127 || n[20] > 128 || n[21] > 128 {
                            return Err(Error::Unsupported(
                                "Nota FLP fuera del rango MIDI admitido",
                            ));
                        }
                        if n[4] != 0 || n[5] != 0 || n[16] != 120 {
                            special_notes += 1;
                        }
                        let mut note = Note::new(tick, length, key as u8, n[21] as f32 / 128.);
                        note.pan = (n[20] as f32 - 64.) / 64.;
                        note.source_id = Some(SourceNoteId::new(total - 1)?);
                        groups
                            .entry((pattern, u16_at(n, 6)))
                            .or_default()
                            .push(note);
                    }
                }
                199 | 159 | 99 | 100 | 233 | 238 => {}
                _ => {
                    unsupported.insert(e.id);
                }
            }
        }
        if groups.len() > 256 * MAX_TRACKS {
            return Err(Error::Limit("Demasiados patrones FLP"));
        }
        let channels: BTreeSet<_> = groups.keys().map(|k| k.1).collect();
        if channels.len() > MAX_TRACKS {
            return Err(Error::Limit("FLP contiene más de 32 canales con notas"));
        }
        let channel_ids: Vec<_> = channels.iter().copied().collect();
        let mut indices = BTreeMap::new();
        for id in channels {
            let index = project.tracks.len();
            indices.insert(id, index);
            project.tracks.push(Track::new(
                names.get(&id).map(String::as_str).unwrap_or("FLP channel"),
                Instrument::Trio,
            ));
        }
        let mut lengths: BTreeMap<u16, u32> = BTreeMap::new();
        for ((pat, _), notes) in &groups {
            let end = notes
                .iter()
                .map(|n| n.tick.saturating_add(n.length))
                .max()
                .unwrap_or(BAR);
            let length = end.div_ceil(BAR).max(1) * BAR;
            lengths
                .entry(*pat)
                .and_modify(|v| *v = (*v).max(length))
                .or_insert(length);
        }
        for (&id, &length) in &explicit_lengths {
            lengths
                .entry(id)
                .and_modify(|v| *v = (*v).max(length))
                .or_insert(length);
        }
        for id in &declared {
            lengths.entry(*id).or_insert(BAR);
        }
        if lengths.len() > 256 {
            return Err(Error::Limit("Demasiados patrones FLP"));
        }
        let mut starts = BTreeMap::new();
        let mut end = 0u32;
        for (&pat, &length) in &lengths {
            starts.insert(pat, end);
            if playlist.is_none() {
                end = end
                    .checked_add(length)
                    .filter(|v| *v <= MAX_TICKS)
                    .ok_or(Error::Limit("FLP demasiado largo"))?;
            }
        }
        project.bars = end.max(BAR).div_ceil(BAR) as u16;
        let mut bindings = Vec::new();
        let mut pattern_ids = BTreeMap::new();
        for ((pat, channel), notes) in groups {
            let index = if let Some(index) = pattern_ids.get(&pat) {
                *index
            } else {
                let index = project.patterns.len();
                project.patterns.push(Pattern {
                    name: pattern_names
                        .get(&pat)
                        .cloned()
                        .unwrap_or_else(|| format!("Pattern {pat}")),
                    length: lengths[&pat],
                    notes: Vec::new(),
                });
                project.clips.push(Clip {
                    offset: 0,
                    muted: false,
                    group: 0,
                    track: 0,
                    pattern: index,
                    tick: starts[&pat],
                    length: lengths[&pat],
                });
                pattern_ids.insert(pat, index);
                index
            };
            for note in notes {
                project.patterns[index]
                    .notes
                    .push(note.on_channel(indices[&channel])?);
            }
            bindings.push(Binding {
                source_pattern: pat,
                channel,
                pattern: index,
            });
        }
        for (&pat, &length) in &lengths {
            if pattern_ids.contains_key(&pat) {
                continue;
            }
            let index = project.patterns.len();
            project.patterns.push(Pattern {
                name: pattern_names
                    .get(&pat)
                    .cloned()
                    .unwrap_or_else(|| format!("Pattern {pat}")),
                length,
                notes: Vec::new(),
            });
            pattern_ids.insert(pat, index);
        }
        if let Some(mut playlist) = playlist {
            project.clips.clear();
            project.arrangement_mutes = playlist.mutes;
            for item in playlist.items {
                if let Some(&pattern) = pattern_ids.get(&item.pattern) {
                    project.bars = project
                        .bars
                        .max((item.tick + item.length).div_ceil(BAR) as u16);
                    project.clips.push(Clip {
                        track: item.track,
                        pattern,
                        tick: item.tick,
                        length: item.length,
                        offset: 0,
                        muted: false,
                        group: item.group,
                    });
                } else {
                    playlist.skipped += 1;
                }
            }
            warnings.push(
                "Playlist FLP importada: posiciones, duración, filas, grupos y filas silenciadas"
                    .into(),
            );
            if playlist.skipped > 0 {
                warnings.push(format!("{} clips de audio/automatización, recortados o con flags no interpretados permanecen opacos", playlist.skipped));
            }
            let empty = project
                .clips
                .iter()
                .filter(|c| project.patterns[c.pattern].notes.is_empty())
                .count();
            if empty > 0 {
                warnings.push(format!(
                    "{empty} clips sin notas MIDI importables; los pasos originales siguen opacos"
                ));
            }
        } else {
            warnings.push(
                "Vista de patrones consecutivos; la playlist original permanece opaca".into(),
            );
        }
        project.source_flp = Some(self.bytes.clone());
        project = project.with_bound_channels()?;
        project.validate()?;
        warnings.push("Instrumentos originales sustituidos por Trio; mixer, automatización y muestras externas siguen opacos".into());
        warnings.push(format!(
            "{} tipos de evento conservados sin interpretar",
            unsupported.len()
        ));
        if special_notes > 0 {
            warnings.push(format!(
                "{special_notes} notas con afinación/flags sin reproducción equivalente"
            ));
        }
        Ok(Import {
            project,
            warnings,
            bindings,
            channel_ids,
        })
    }
    pub fn update_notes_and_tempo(&self, project: &Project) -> Result<Vec<u8>> {
        project.validate()?;
        let imported = self.import()?;
        if project.arrangement_mutes != imported.project.arrangement_mutes
            || project.rate != imported.project.rate
            || project
                .patterns
                .iter()
                .zip(&imported.project.patterns)
                .any(|(a, b)| a.name != b.name || a.length != b.length)
            || project.tracks.len() != imported.project.tracks.len()
            || project.patterns.len() != imported.project.patterns.len()
            || project.clips != imported.project.clips
            || project.audio_clips != imported.project.audio_clips
            || project.tempos != imported.project.tempos
            || project.tracks != imported.project.tracks
            || project.routes != imported.project.routes
            || project.master != imported.project.master
            || project.swing != imported.project.swing
            || project.title != imported.project.title
            || project.markers != imported.project.markers
            || project.automation != imported.project.automation
            || project.bars != imported.project.bars
        {
            return Err(Error::Unsupported(
                "FLP: guarda .gstudio para conservar cambios de canales/arreglo/tempo variable",
            ));
        }
        for (index, pattern) in project.patterns.iter().enumerate() {
            if !imported.bindings.iter().any(|b| b.pattern == index)
                && pattern.notes != imported.project.patterns[index].notes
            {
                return Err(Error::Unsupported(
                    "FLP: guarda .gstudio para añadir notas a patrones vacíos",
                ));
            }
        }
        let mut replacements = BTreeMap::new();
        let mut bindings_by_pattern: BTreeMap<u16, Vec<&Binding>> = BTreeMap::new();
        for binding in &imported.bindings {
            bindings_by_pattern
                .entry(binding.source_pattern)
                .or_default()
                .push(binding);
        }
        let changed_patterns: BTreeSet<_> = bindings_by_pattern
            .iter()
            .filter_map(|(&source, bindings)| {
                let binding = bindings.first()?;
                (project.patterns[binding.pattern].notes
                    != imported.project.patterns[binding.pattern].notes)
                    .then_some(source)
            })
            .collect();
        let mut current = 1u16;
        let mut written = BTreeSet::new();
        let source_notes: Vec<_> = self
            .events
            .iter()
            .filter(|event| event.id == 224)
            .flat_map(|event| self.value(event).as_chunks::<24>().0.iter().copied())
            .collect();
        for (i, e) in self.events.iter().enumerate() {
            let data = self.value(e);
            match e.id {
                65 => current = u16_at(data, 0),
                156 => {
                    if project.bpm != imported.project.bpm {
                        replacements.insert(
                            i,
                            ((project.bpm * 1000.).round() as u32)
                                .to_le_bytes()
                                .to_vec(),
                        );
                    }
                }
                66 => {
                    if project.bpm != imported.project.bpm {
                        replacements.insert(i, (project.bpm.round() as u16).to_le_bytes().to_vec());
                    }
                }
                224 => {
                    if !changed_patterns.contains(&current) {
                        continue;
                    }
                    let bindings = bindings_by_pattern
                        .get(&current)
                        .ok_or(Error::Invalid("Falta el patrón fuente"))?;
                    if data
                        .as_chunks::<24>()
                        .0
                        .iter()
                        .any(|n| n[4] != 0 || n[5] != 0 || n[16] != 120)
                    {
                        return Err(Error::Unsupported(
                            "FLP: conserva las notas slide/afinadas sin editar o guarda .gstudio",
                        ));
                    }
                    if !written.insert(current) {
                        return Err(Error::Unsupported(
                            "FLP con varios bloques de notas por patrón: exportación editada no disponible",
                        ));
                    }
                    let mut out = Vec::new();
                    let binding = bindings
                        .first()
                        .ok_or(Error::Invalid("Falta el patrón fuente"))?;
                    for n in &project.patterns[binding.pattern].notes {
                        let channel = if let Some(channel) = n.channel {
                            *imported
                                .channel_ids
                                .get(channel.index())
                                .ok_or(Error::Invalid("Canal FLP no disponible"))?
                        } else if bindings.len() == 1 {
                            binding.channel
                        } else {
                            return Err(Error::Unsupported(
                                "Asigna un canal a la nota antes de guardar FLP",
                            ));
                        };
                        let mut record = if let Some(id) = n.source_id {
                            *source_notes
                                .get(id.index())
                                .ok_or(Error::Invalid("La nota fuente ya no existe"))?
                        } else {
                            let mut record = [0u8; 24];
                            record[16] = 120;
                            record[18] = 64;
                            record[22] = 128;
                            record[23] = 128;
                            record
                        };
                        let tick = (n.tick as u64 * self.ppq as u64 + PPQ as u64 / 2) / PPQ as u64;
                        let length =
                            (n.length as u64 * self.ppq as u64 + PPQ as u64 / 2) / PPQ as u64;
                        record[..4].copy_from_slice(&(tick as u32).to_le_bytes());
                        record[6..8].copy_from_slice(&channel.to_le_bytes());
                        record[8..12].copy_from_slice(&(length.max(1) as u32).to_le_bytes());
                        record[12..14].copy_from_slice(&(n.key as u16).to_le_bytes());
                        record[20] = ((n.pan + 1.) * 64.).round() as u8;
                        record[21] = (n.velocity * 128.).round() as u8;
                        out.extend(record);
                    }
                    replacements.insert(i, out);
                }
                _ => {}
            }
        }
        self.rewrite(&replacements)
    }
}

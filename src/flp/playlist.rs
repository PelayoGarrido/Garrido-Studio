use super::*;

pub(super) struct Item {
    pub pattern: u16,
    pub track: usize,
    pub tick: u32,
    pub length: u32,
    pub group: u16,
}
pub(super) struct Playlist {
    pub items: Vec<Item>,
    pub mutes: u32,
    pub skipped: usize,
}

impl Document {
    pub(super) fn playlist(&self, warnings: &mut Vec<String>) -> Result<Option<Playlist>> {
        let version = self.events.iter().find(|e| e.id == 199).and_then(|e| {
            let raw = std::str::from_utf8(self.value(e))
                .ok()?
                .trim_end_matches('\0');
            let parts = raw
                .split('.')
                .take(3)
                .map(str::parse::<u16>)
                .collect::<std::result::Result<Vec<_>, _>>()
                .ok()?;
            (parts.len() == 3).then(|| (parts[0], parts[1], parts[2]))
        });
        let selected = self
            .events
            .iter()
            .rev()
            .find(|e| e.id == 100)
            .map_or(0, |e| u16_at(self.value(e), 0));
        let mut current = 0;
        let mut payload = None;
        let mut mutes = 0u32;
        let mut row = 0usize;
        let mut arrangements = BTreeSet::new();
        for event in &self.events {
            let data = self.value(event);
            if event.id == 99 {
                current = u16_at(data, 0);
                arrangements.insert(current);
                row = 0;
            }
            if current != selected {
                continue;
            }
            if event.id == 233 {
                if payload.replace(data).is_some() {
                    warnings.push(
                        "Playlist con bloques repetidos: se mantiene la vista de patrones".into(),
                    );
                    return Ok(None);
                }
            } else if event.id == 238 {
                if row < MAX_LANES && data.len() >= 13 && data[12] == 0 {
                    mutes |= 1 << row;
                }
                row += 1;
            }
        }
        let Some(data) = payload else {
            return Ok(None);
        };
        if arrangements.len() > 1 {
            warnings.push(format!(
                "Se importa el arreglo {selected}; los demás permanecen en el FLP original"
            ));
        }
        if data.is_empty() {
            return Ok(Some(Playlist {
                items: Vec::new(),
                mutes,
                skipped: 0,
            }));
        }
        if !version.is_some_and(|v| v >= (12, 9, 1) && v.0 <= 20) {
            warnings.push(
                "Estructura de playlist sin validar para esta versión: vista de patrones".into(),
            );
            return Ok(None);
        }
        if !data.len().is_multiple_of(32) {
            return Err(Error::Unsupported(
                "Playlist FLP truncada o con estructura distinta de 32 bytes",
            ));
        }
        if data.len() / 32 > 4096 {
            return Err(Error::Limit("Demasiados clips FLP"));
        }
        let scale = |tick: u32| -> Result<u32> {
            let value = (tick as u64 * PPQ as u64 + self.ppq as u64 / 2) / self.ppq as u64;
            if value > MAX_TICKS as u64 {
                Err(Error::Limit("Playlist FLP supera 256 compases"))
            } else {
                Ok(value as u32)
            }
        };
        let mut playlist = Playlist {
            items: Vec::new(),
            mutes,
            skipped: 0,
        };
        for record in data.as_chunks::<32>().0 {
            let base = u16_at(record, 4);
            let index = u16_at(record, 6);
            let row = 499u16.checked_sub(u16_at(record, 12)).map(usize::from);
            let offset = u32_at(record, 24);
            if base != 20480
                || index <= base
                || row.is_none_or(|r| r >= MAX_LANES)
                || u16_at(record, 18) != 64
                || (offset != 0 && offset != u32::MAX)
            {
                playlist.skipped += 1;
                continue;
            }
            let tick = scale(u32_at(record, 0))?;
            let length = scale(u32_at(record, 8))?;
            if length == 0 || tick.checked_add(length).is_none_or(|end| end > MAX_TICKS) {
                return Err(Error::Invalid("Clip FLP fuera del arreglo"));
            }
            playlist.items.push(Item {
                pattern: index - base,
                track: row.unwrap_or(0),
                tick,
                length,
                group: u16_at(record, 14),
            });
        }
        Ok(Some(playlist))
    }
}

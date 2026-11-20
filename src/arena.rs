use crate::wire::{Cursor, Packet, WriteWire, invalid};
use bytes::{BufMut, BytesMut};
use std::{
    collections::BTreeMap,
    io::{self, Read},
    path::Path,
    sync::{Arc, OnceLock},
};

const MAGIC: &[u8] = b"CORAL47\x01";
const MAX_TEMPLATE: usize = 64 << 20;
pub type ColumnPos = (i32, i32);
pub type BlockPos = (i32, u8, i32);

pub(crate) fn column(pos: [f64; 3]) -> ColumnPos {
    (pos[0].floor() as i32 >> 4, pos[2].floor() as i32 >> 4)
}

#[derive(Clone)]
pub(crate) struct Section {
    pub(crate) palette: Vec<u16>,
    pub(crate) indices: Vec<u8>,
    pub(crate) light: Option<Box<[[u8; 2048]; 2]>>,
}
impl Section {
    fn bits(&self) -> usize {
        (usize::BITS - (self.palette.len() - 1).leading_zeros()) as usize
    }
    pub(crate) fn state(&self, i: usize) -> u16 {
        let width = self.bits();
        let bit = i * width;
        let mut word = 0u32;
        for (shift, byte) in self.indices[bit / 8..].iter().take(3).enumerate() {
            word |= u32::from(*byte) << (8 * shift);
        }
        self.palette[((word >> (bit % 8)) & ((1 << width) - 1)) as usize]
    }
    fn write_states(&self, dst: &mut BytesMut) {
        let start = dst.len();
        dst.resize(start + 8192, 0);
        let output = &mut dst[start..];
        let width = self.bits();
        if width == 0 {
            let state = self.palette[0].to_le_bytes();
            if state != [0, 0] {
                for block in output.as_chunks_mut::<2>().0 {
                    *block = state;
                }
            }
            return;
        }

        let mask = (1u32 << width) - 1;
        let mut bytes = self.indices.iter();
        let mut word = 0u32;
        let mut available = 0;
        for block in output.as_chunks_mut::<2>().0 {
            while available < width {
                // Section loading validates a packed length of 512 * width.
                let Some(&byte) = bytes.next() else {
                    unreachable!("validated section length")
                };
                word |= u32::from(byte) << available;
                available += 8;
            }
            *block = self.palette[(word & mask) as usize].to_le_bytes();
            word >>= width;
            available -= width;
        }
    }

    fn read(r: &mut Cursor<'_>) -> io::Result<Self> {
        let n = r.short()? as usize;
        if !(1..=4096).contains(&n) {
            return Err(invalid("invalid arena palette"));
        }
        let palette = (0..n)
            .map(|_| r.short().map(|v| v as u16))
            .collect::<io::Result<Vec<_>>>()?;
        let mut s = Self {
            palette,
            indices: Vec::new(),
            light: None,
        };
        let width = s.bits();
        s.indices = r.take(512 * width)?.to_vec();
        if !n.is_power_of_two() {
            for i in 0..4096 {
                let bit = i * width;
                let mut v = 0u32;
                for (j, b) in s.indices[bit / 8..].iter().take(3).enumerate() {
                    v |= u32::from(*b) << (8 * j);
                }
                if ((v >> (bit % 8)) & ((1 << width) - 1)) as usize >= n {
                    return Err(invalid("arena palette index outside table"));
                }
            }
        }
        s.light = match r.byte()? {
            0 => None,
            1 => Some(Box::new([
                r.take(2048)?.try_into().unwrap(),
                r.take(2048)?.try_into().unwrap(),
            ])),
            _ => return Err(invalid("unknown light encoding")),
        };
        Ok(s)
    }
}

pub(crate) struct Column {
    pub(crate) sections: BTreeMap<u8, Section>,
    pub(crate) biomes: [u8; 256],
    pub(crate) packet: OnceLock<Packet>,
    pub(crate) nbt: Vec<u8>,
}
impl Column {
    fn packet(&self, (x, z): ColumnPos) -> Packet {
        self.packet
            .get_or_init(|| {
                let payload_len = self.sections.len() * 12288 + 256;
                Packet::with_capacity(0x21, payload_len + 15, |w| {
                    w.put_i32(x);
                    w.put_i32(z);
                    w.put_u8(1);
                    let mask = self.sections.keys().fold(0u16, |m, y| m | (1 << *y));
                    w.put_u16(mask);
                    w.var(payload_len as i32);
                    for s in self.sections.values() {
                        s.write_states(w);
                    }
                    for plane in 0..2 {
                        for s in self.sections.values() {
                            if let Some(light) = &s.light {
                                w.extend_from_slice(&light[plane]);
                            } else {
                                w.resize(w.len() + 2048, if plane == 0 { 0 } else { 255 });
                            }
                        }
                    }
                    w.extend_from_slice(&self.biomes);
                })
            })
            .clone()
    }
}

pub struct Template {
    pub(crate) columns: BTreeMap<ColumnPos, Column>,
}
impl Template {
    pub fn flat(radius: i32) -> Self {
        let mut columns = BTreeMap::new();
        for x in -radius..=radius {
            for z in -radius..=radius {
                let mut indices = vec![0; 1024]; // four entries per byte, indexed in YZX order
                indices[..64].fill(0x55);
                indices[64..192].fill(0xaa);
                indices[192..256].fill(0xff);
                let section = Section {
                    palette: vec![0, 7 << 4, 3 << 4, 2 << 4],
                    indices,
                    light: None,
                };
                columns.insert(
                    (x, z),
                    Column {
                        sections: BTreeMap::from([(0, section)]),
                        biomes: [1; 256],
                        packet: OnceLock::new(),
                        nbt: Vec::new(),
                    },
                );
            }
        }
        Self { columns }
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(MAGIC);
        buf.put_u32(self.columns.len() as u32);
        for (&(x, z), col) in &self.columns {
            buf.put_i32(x);
            buf.put_i32(z);
            buf.put_u8(col.sections.len() as u8);
            for (&y, sec) in &col.sections {
                buf.put_u8(y);
                buf.put_u16(sec.palette.len() as u16);
                for &state in &sec.palette {
                    buf.put_u16(state);
                }
                buf.extend_from_slice(&sec.indices);
                buf.put_u8(sec.light.is_some() as u8);
                if let Some(light) = &sec.light {
                    buf.extend_from_slice(&light[0]);
                    buf.extend_from_slice(&light[1]);
                }
            }
            buf.extend_from_slice(&col.biomes);
            buf.put_u32(col.nbt.len() as u32);
            buf.extend_from_slice(&col.nbt);
            if buf.len() > MAX_TEMPLATE {
                return Err(invalid("arena exceeds size limit"));
            }
        }
        let encoded = zstd::bulk::compress(&buf, 5)?;
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, &encoded))
    }
    pub fn load(path: &Path) -> io::Result<Self> {
        let input = std::fs::File::open(path)?;
        if input.metadata()?.len() > MAX_TEMPLATE as u64 {
            return Err(invalid("compressed arena too large"));
        }
        let mut decoder = zstd::stream::read::Decoder::new(input)?;
        decoder.window_log_max(26)?;
        let mut data = Vec::new();
        decoder
            .take(MAX_TEMPLATE as u64 + 1)
            .read_to_end(&mut data)?;
        if data.len() > MAX_TEMPLATE {
            return Err(invalid("arena exceeds size limit"));
        }
        let mut r = Cursor(&data);
        if r.take(MAGIC.len())? != MAGIC {
            return Err(invalid("unsupported arena format"));
        }
        let count = r.int()?;
        if !(1..=16384).contains(&count) {
            return Err(invalid("invalid column count"));
        }
        let mut columns = BTreeMap::new();
        for _ in 0..count {
            let pos = (r.int()?, r.int()?);
            let n = r.byte()?;
            if n > 16 || pos.0.unsigned_abs() > 1_875_000 || pos.1.unsigned_abs() > 1_875_000 {
                return Err(invalid("invalid column"));
            }
            let mut sections = BTreeMap::new();
            for _ in 0..n {
                let y = r.byte()?;
                if y >= 16 || sections.insert(y, Section::read(&mut r)?).is_some() {
                    return Err(invalid("invalid section index"));
                }
            }
            let biomes = r.take(256)?.try_into().unwrap();
            let nbt_len = r.int()? as usize;
            if nbt_len > 4 << 20 {
                return Err(invalid("chunk NBT exceeds limit"));
            }
            let nbt = r.take(nbt_len)?.to_vec();
            if columns
                .insert(
                    pos,
                    Column {
                        sections,
                        biomes,
                        packet: OnceLock::new(),
                        nbt,
                    },
                )
                .is_some()
            {
                return Err(invalid("duplicate column"));
            }
        }
        r.end()?;
        Ok(Self { columns })
    }
    pub fn block(&self, (x, y, z): BlockPos) -> u16 {
        self.columns
            .get(&(x >> 4, z >> 4))
            .and_then(|c| c.sections.get(&(y >> 4)))
            .map_or(0, |s| {
                s.state(usize::from(y & 15) * 256 + (z & 15) as usize * 16 + (x & 15) as usize)
            })
    }
    pub fn spawn(&self) -> io::Result<[f64; 3]> {
        for (cx, cz) in std::iter::once((0, 0)).chain(self.columns.keys().copied()) {
            let (x, z) = (cx * 16, cz * 16);
            if let Some(y) = (0..=252).rev().find(|&y| {
                self.block((x, y, z)) != 0
                    && self.block((x, y + 1, z)) == 0
                    && self.block((x, y + 2, z)) == 0
            }) {
                return Ok([x as f64 + 0.5, y as f64 + 1., z as f64 + 0.5]);
            }
        }
        Err(invalid("arena has no standing spawn"))
    }
    pub fn packet(&self, pos: ColumnPos) -> Option<Packet> {
        self.columns.get(&pos).map(|col| col.packet(pos))
    }
    pub(crate) fn bulk_packet(&self, positions: [ColumnPos; 2]) -> Option<Packet> {
        let [a, b] = positions;
        let columns = [self.columns.get(&a)?, self.columns.get(&b)?];
        let lengths = columns.map(|col| col.sections.len() * 12288 + 256);
        Some(Packet::with_capacity(
            0x26,
            23 + lengths.iter().sum::<usize>(),
            |w| {
                w.put_u8(1); // Overworld sky light
                w.var(2);
                for (&(x, z), col) in positions.iter().zip(columns) {
                    w.put_i32(x);
                    w.put_i32(z);
                    w.put_u16(col.sections.keys().fold(0u16, |mask, y| mask | (1 << y)));
                }
                for ((col, pos), len) in columns.into_iter().zip(positions).zip(lengths) {
                    let packet = col.packet(pos);
                    let body = packet.body();
                    w.extend_from_slice(&body[body.len() - len..]);
                }
            },
        ))
    }

    pub fn cached(&self, pos: ColumnPos) -> bool {
        self.columns
            .get(&pos)
            .is_some_and(|c| c.packet.get().is_some())
    }
    pub fn contains(&self, pos: ColumnPos) -> bool {
        self.columns.contains_key(&pos)
    }
}

pub struct World {
    pub name: String,
    pub template: Arc<Template>,
    pub edits: Arc<crate::anvil::Edits>,
}
impl World {
    pub fn new(name: String, template: Arc<Template>) -> Self {
        Self {
            name,
            template,
            edits: Arc::default(),
        }
    }
    pub fn block(&self, p: BlockPos) -> u16 {
        self.edits
            .get(&(p.0 >> 4, p.2 >> 4))
            .and_then(|m| m.get(&p))
            .copied()
            .unwrap_or_else(|| self.template.block(p))
    }
    pub fn set(&mut self, p: BlockPos, state: u16) -> io::Result<()> {
        let col = (p.0 >> 4, p.2 >> 4);
        if !self.template.contains(col) {
            return Err(invalid("outside arena"));
        }
        let edits = Arc::make_mut(&mut self.edits);
        if self.template.block(p) == state {
            if let Some(column) = edits.get_mut(&col) {
                column.remove(&p);
                if column.is_empty() {
                    edits.remove(&col);
                }
            }
        } else {
            edits.entry(col).or_default().insert(p, state);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_wire_bytes_match_all_palette_widths() {
        for width in 0..=12 {
            let n = if width < 2 {
                1 << width
            } else {
                (1 << width) - 1
            };
            let palette: Vec<u16> = (0..n).map(|v| (v as u16).wrapping_mul(311)).collect();
            let mut indices = vec![0u8; 512 * width];
            let mut expected = Vec::new();
            for i in 0..4096 {
                let index = (i * 199 + 13) % n;
                expected.extend_from_slice(&palette[index].to_le_bytes());
                for bit in 0..width {
                    let offset = i * width + bit;
                    indices[offset / 8] |= (((index >> bit) & 1) << (offset % 8)) as u8;
                }
            }
            let section = Section {
                palette,
                indices,
                light: None,
            };
            let mut actual = BytesMut::new();
            section.write_states(&mut actual);
            assert_eq!(actual.as_ref(), expected, "width {width}");
        }
    }
}

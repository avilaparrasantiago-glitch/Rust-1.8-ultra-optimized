use crate::{
    arena::{BlockPos, Column, ColumnPos, Section, Template},
    wire::invalid,
};
use fastnbt::{ByteArray, IntArray, Value};
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{self, Read, Write},
    path::Path,
    sync::OnceLock,
};

const SECTOR: usize = 4096;
const MAX_NBT: usize = 4 << 20;
type Compound = HashMap<String, Value>;
pub type Edits = BTreeMap<ColumnPos, BTreeMap<BlockPos, u16>>;

fn compound(value: &Value) -> io::Result<&Compound> {
    if let Value::Compound(c) = value {
        Ok(c)
    } else {
        Err(invalid("expected NBT compound"))
    }
}
fn bytes<const N: usize>(c: &Compound, name: &str) -> io::Result<[u8; N]> {
    let Some(Value::ByteArray(a)) = c.get(name) else {
        return Err(invalid("missing chunk array"));
    };
    if a.len() != N {
        return Err(invalid("wrong chunk array length"));
    }
    Ok(std::array::from_fn(|i| a[i] as u8))
}
fn array(v: &[u8]) -> Value {
    Value::ByteArray(ByteArray::new(v.iter().map(|b| *b as i8).collect()))
}
fn nibble(v: &[u8], i: usize) -> u16 {
    u16::from((v[i >> 1] >> (4 * (i & 1))) & 15)
}
fn put_nibble(v: &mut [u8], i: usize, n: u16) {
    let shift = 4 * (i & 1);
    v[i >> 1] = (v[i >> 1] & !(15 << shift)) | (((n & 15) as u8) << shift);
}
fn parse(data: &[u8]) -> io::Result<Value> {
    fastnbt::from_bytes_with_opts(data, fastnbt::DeOpts::new().max_seq_len(1 << 20))
        .map_err(io::Error::other)
}

fn column(data: Vec<u8>, pos: ColumnPos) -> io::Result<Column> {
    let root = parse(&data)?;
    let level = compound(
        compound(&root)?
            .get("Level")
            .ok_or(invalid("missing Level"))?,
    )?;
    if level.get("xPos") != Some(&Value::Int(pos.0))
        || level.get("zPos") != Some(&Value::Int(pos.1))
    {
        return Err(invalid("region location does not match chunk coordinates"));
    }
    let Some(Value::List(list)) = level.get("Sections") else {
        return Err(invalid("missing Sections"));
    };
    let biomes = bytes(level, "Biomes")?;
    let mut sections = BTreeMap::new();
    for value in list {
        let sec = compound(value)?;
        let Some(Value::Byte(y)) = sec.get("Y") else {
            return Err(invalid("missing Y"));
        };
        if !(0..16).contains(y) || sections.contains_key(&(*y as u8)) {
            return Err(invalid("duplicate or invalid section"));
        }
        let ids = bytes::<4096>(sec, "Blocks")?;
        let meta = bytes::<2048>(sec, "Data")?;
        let add = if sec.contains_key("Add") {
            Some(bytes::<2048>(sec, "Add")?)
        } else {
            None
        };
        let light = Box::new([
            bytes(sec, "BlockLight")?,
            if sec.contains_key("SkyLight") {
                bytes(sec, "SkyLight")?
            } else {
                [0; 2048]
            },
        ]);
        let mut table = HashMap::new();
        let mut palette = Vec::new();
        let mut indices = [0usize; 4096];
        for (i, out) in indices.iter_mut().enumerate() {
            let high = add.as_ref().map_or(0, |v| nibble(v, i));
            let state = (high << 12) | (u16::from(ids[i]) << 4) | nibble(&meta, i);
            *out = *table.entry(state).or_insert_with(|| {
                palette.push(state);
                palette.len() - 1
            });
        }
        let width = (usize::BITS - (palette.len() - 1).leading_zeros()) as usize;
        let mut packed = vec![0; 512 * width];
        for (i, index) in indices.into_iter().enumerate() {
            for bit in 0..width {
                let at = i * width + bit;
                packed[at / 8] |= (((index >> bit) & 1) as u8) << (at % 8);
            }
        }
        sections.insert(
            *y as u8,
            Section {
                palette,
                indices: packed,
                light: Some(light),
            },
        );
    }
    Ok(Column {
        skylight: true,
        sections,
        biomes,
        nbt: data,
        packet: OnceLock::new(),
    })
}

fn inflate(kind: u8, payload: &[u8]) -> io::Result<Vec<u8>> {
    if kind == 1 {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(payload)
            .take((MAX_NBT + 1) as u64)
            .read_to_end(&mut out)?;
        if out.len() > MAX_NBT {
            return Err(invalid("oversized chunk"));
        }
        return Ok(out);
    }
    if kind != 2 {
        return Err(invalid("unsupported Anvil compression"));
    }
    let mut d = libdeflater::Decompressor::new();
    let mut out = vec![0; 65536];
    loop {
        match d.zlib_decompress(payload, &mut out) {
            Ok(n) => {
                out.truncate(n);
                return Ok(out);
            }
            Err(libdeflater::DecompressionError::InsufficientSpace) if out.len() < MAX_NBT => {
                out.resize(out.len() * 2, 0)
            }
            _ => return Err(invalid("invalid or oversized Anvil payload")),
        }
    }
}

pub fn import(dir: &Path) -> io::Result<Template> {
    let mut columns = BTreeMap::new();
    let mut total = 0usize;
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("mca") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or(invalid("invalid region name"))?;
        let mut parts = name.split('.');
        if parts.next() != Some("r") {
            continue;
        }
        let mut coord = || -> io::Result<i32> {
            parts
                .next()
                .and_then(|v| v.parse::<i32>().ok())
                .filter(|v| v.unsigned_abs() <= 58594)
                .ok_or(invalid("invalid region coordinate"))
        };
        let (rx, rz) = (coord()?, coord()?);
        if parts.next().is_some() {
            return Err(invalid("invalid region name"));
        }
        let file = File::open(&path)?;
        let len = file.metadata()?.len();
        if !(8192..=512 << 20).contains(&len) || len % 4096 != 0 {
            return Err(invalid("invalid region size"));
        }
        // Map a private snapshot. External writers can truncate the source while an import is running.
        let mut snapshot = memmap2::MmapMut::map_anon(len as usize)?;
        (&file).read_exact(&mut snapshot)?;
        let map = snapshot.make_read_only()?;
        let mut occupied = vec![false; map.len() / SECTOR];
        occupied[..2].fill(true);
        for i in 0..1024 {
            let h = u32::from_be_bytes(map[i * 4..i * 4 + 4].try_into().unwrap());
            if h == 0 {
                continue;
            }
            let first = (h >> 8) as usize;
            let sectors = (h & 255) as usize;
            let range = occupied
                .get_mut(first..first + sectors)
                .ok_or(invalid("sector outside region"))?;
            if sectors == 0 || range.iter().any(|v| *v) {
                return Err(invalid("overlapping sector allocation"));
            }
            range.fill(true);
            let chunk = &map[first * SECTOR..(first + sectors) * SECTOR];
            let n = u32::from_be_bytes(chunk[..4].try_into().unwrap()) as usize;
            if n < 2 || n > chunk.len() - 4 {
                return Err(invalid("invalid chunk length"));
            }
            let pos = (rx * 32 + (i % 32) as i32, rz * 32 + (i / 32) as i32);
            let data = inflate(chunk[4], &chunk[5..4 + n])?;
            total += data.len();
            if total > 64 << 20 {
                return Err(invalid("arena import exceeds 64 MiB of NBT"));
            }
            columns.insert(pos, column(data, pos)?);
        }
    }
    if columns.is_empty() {
        return Err(invalid("no Anvil chunks found"));
    }
    Ok(Template {
        spawn_hint: None,
        columns,
    })
}
fn encode_column(
    pos: ColumnPos,
    col: &Column,
    changes: Option<&BTreeMap<BlockPos, u16>>,
) -> io::Result<Vec<u8>> {
    if changes.is_none_or(|c| c.is_empty()) && !col.nbt.is_empty() {
        return Ok(col.nbt.clone());
    }
    let mut root = if col.nbt.is_empty() {
        Compound::new()
    } else {
        compound(&parse(&col.nbt)?)?.clone()
    };
    let mut level = match root.remove("Level") {
        Some(Value::Compound(v)) => v,
        _ => Compound::new(),
    };
    level.insert("xPos".into(), Value::Int(pos.0));
    level.insert("zPos".into(), Value::Int(pos.1));
    level.insert("Biomes".into(), array(&col.biomes));
    level
        .entry("Entities".into())
        .or_insert_with(|| Value::List(vec![]));
    level
        .entry("TileEntities".into())
        .or_insert_with(|| Value::List(vec![]));
    level
        .entry("TerrainPopulated".into())
        .or_insert(Value::Byte(1));
    let mut originals = HashMap::new();
    if let Some(Value::List(list)) = level.remove("Sections") {
        for v in list {
            if let Value::Compound(c) = v
                && let Some(Value::Byte(y)) = c.get("Y")
            {
                originals.insert(*y as u8, c);
            }
        }
    }
    let mut sections = Vec::new();
    let mut heights = [0; 256];
    for y in 0..16u8 {
        let section = col.sections.get(&y);
        let edited = changes.is_some_and(|c| c.keys().any(|p| p.1 >> 4 == y));
        if section.is_none() && !edited {
            continue;
        }
        let mut ids = [0; 4096];
        let mut meta = [0; 2048];
        let mut add = [0; 2048];
        for (i, id) in ids.iter_mut().enumerate() {
            let p = (
                pos.0 * 16 + (i & 15) as i32,
                y * 16 + (i >> 8) as u8,
                pos.1 * 16 + ((i >> 4) & 15) as i32,
            );
            let state = changes
                .and_then(|c| c.get(&p).copied())
                .unwrap_or_else(|| section.map_or(0, |s| s.state(i)));
            *id = (state >> 4) as u8;
            put_nibble(&mut meta, i, state);
            put_nibble(&mut add, i, state >> 12);
            if state >> 4 != 0 {
                heights[i & 255] = i32::from(p.1) + 1;
            }
        }
        let mut c = originals.remove(&y).unwrap_or_default();
        c.insert("Y".into(), Value::Byte(y as i8));
        c.insert("Blocks".into(), array(&ids));
        c.insert("Data".into(), array(&meta));
        if add.iter().any(|&x| x != 0) {
            c.insert("Add".into(), array(&add));
        } else {
            c.remove("Add");
        }
        let light = section.and_then(|s| s.light.as_deref());
        c.entry("BlockLight".into())
            .or_insert_with(|| array(light.map_or(&[0; 2048], |v| &v[0])));
        if col.skylight {
            c.entry("SkyLight".into())
                .or_insert_with(|| array(light.map_or(&[255; 2048], |v| &v[1])));
        } else {
            c.remove("SkyLight");
        }
        sections.push(Value::Compound(c));
    }
    if let Some(changes) = changes
        && let Some(Value::List(tiles)) = level.get_mut("TileEntities")
    {
        tiles.retain(|v| {
            let Ok(c) = compound(v) else {
                return true;
            };
            match (c.get("x"), c.get("y"), c.get("z")) {
                (Some(Value::Int(x)), Some(Value::Int(y)), Some(Value::Int(z))) => {
                    !u8::try_from(*y).is_ok_and(|y| changes.contains_key(&(*x, y, *z)))
                }
                _ => true,
            }
        });
    }
    level.insert("Sections".into(), Value::List(sections));
    level.insert(
        "HeightMap".into(),
        Value::IntArray(IntArray::new(heights.to_vec())),
    );
    root.insert("Level".into(), Value::Compound(level));
    fastnbt::to_bytes(&root).map_err(io::Error::other)
}

/// The destination directory belongs to this template. Each published region is a complete snapshot.
pub fn export(dir: &Path, template: &Template, edits: &Edits) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(".coral-writer"))?;
    lock.try_lock().map_err(io::Error::other)?;
    let mut regions: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (&pos, col) in &template.columns {
        regions
            .entry((pos.0 >> 5, pos.1 >> 5))
            .or_default()
            .push((pos, col));
    }
    let mut compressor = libdeflater::Compressor::new(libdeflater::CompressionLvl::default());
    let mut scratch = Vec::new();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;
    for ((rx, rz), cols) in regions {
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        let mut header = [0u8; 8192];
        file.write_all(&header)?;
        let mut sector = 2usize;
        for (pos, col) in cols {
            let raw = encode_column(pos, col, edits.get(&pos))?;
            scratch.resize(compressor.zlib_compress_bound(raw.len()), 0);
            let n = compressor
                .zlib_compress(&raw, &mut scratch)
                .map_err(io::Error::other)?;
            let count = (5 + n).div_ceil(SECTOR);
            if count > 255 || sector >= 1 << 24 {
                return Err(invalid("chunk exceeds Anvil sector limit"));
            }
            let i = (((pos.1 & 31) * 32 + (pos.0 & 31)) * 4) as usize;
            let location = ((sector as u32) << 8) | count as u32;
            header[i..i + 4].copy_from_slice(&location.to_be_bytes());
            header[4096 + i..4100 + i].copy_from_slice(&timestamp.to_be_bytes());
            file.write_all(&((n + 1) as u32).to_be_bytes())?;
            file.write_all(&[2])?;
            file.write_all(&scratch[..n])?;
            file.write_all(&[0; 4096][..count * SECTOR - n - 5])?;
            sector += count;
        }
        std::io::Seek::rewind(&mut file)?;
        file.write_all(&header)?;
        file.as_file().sync_all()?;
        file.persist(dir.join(format!("r.{rx}.{rz}.mca")))
            .map_err(|e| e.error)?;
    }
    Ok(())
}

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
pub(crate) struct Spawner {
    pub pos: BlockPos,
    pub update: crate::wire::Packet,
    data: Vec<u8>,
}
impl Spawner {
    fn with_update(pos: BlockPos, data: Vec<u8>, client_data: &[u8]) -> Self {
        use bytes::BufMut;
        let update = crate::wire::Packet::new(0x35, |w| {
            w.put_i64(crate::packets::packed(pos));
            w.put_u8(1);
            w.extend_from_slice(client_data);
        });
        Self { pos, data, update }
    }
    pub(crate) fn blaze(pos: BlockPos) -> Self {
        use bytes::BufMut;
        fn name(out: &mut Vec<u8>, s: &str) {
            out.put_u16(s.len() as u16);
            out.extend_from_slice(s.as_bytes());
        }
        let mut data = vec![10, 0, 0]; // NBT root compound; strings below are ASCII.
        for (key, value) in [("id", "MobSpawner"), ("EntityId", "Blaze")] {
            data.put_u8(8);
            name(&mut data, key);
            name(&mut data, value);
        }
        for (key, value) in [("x", pos.0), ("y", i32::from(pos.1)), ("z", pos.2)] {
            data.put_u8(3);
            name(&mut data, key);
            data.put_i32(value);
        }
        for (key, value) in [
            ("Delay", 20),
            ("MinSpawnDelay", 200),
            ("MaxSpawnDelay", 800),
            ("SpawnCount", 4),
            ("MaxNearbyEntities", 6),
            ("RequiredPlayerRange", 16),
            ("SpawnRange", 4),
        ] {
            data.put_u8(2);
            name(&mut data, key);
            data.put_i16(value);
        }
        data.put_u8(0);
        Self::with_update(pos, data.clone(), &data)
    }
}

fn spawners_in_level(pos: ColumnPos, level: &Compound) -> io::Result<Vec<Spawner>> {
    let mut out = Vec::new();
    if let Some(Value::List(tiles)) = level.get("TileEntities") {
        for tile in tiles {
            let Value::Compound(tag) = tile else {
                continue;
            };
            if !matches!(tag.get("id"),Some(Value::String(id)) if id=="MobSpawner") {
                continue;
            }
            let (Some(Value::Int(x)), Some(Value::Int(y)), Some(Value::Int(z))) =
                (tag.get("x"), tag.get("y"), tag.get("z"))
            else {
                return Err(invalid("missing spawner position"));
            };
            let y = u8::try_from(*y).map_err(|_| invalid("invalid spawner height"))?;
            if (*x >> 4, *z >> 4) != pos {
                return Err(invalid("spawner outside chunk"));
            }
            let mut client_tag = tag.clone();
            client_tag.remove("SpawnPotentials");
            let data = fastnbt::to_bytes(tile).map_err(io::Error::other)?;
            let wire = fastnbt::to_bytes(&client_tag).map_err(io::Error::other)?;
            out.push(Spawner::with_update((*x, y, *z), data, &wire));
        }
    }
    Ok(out)
}

pub(crate) fn saved_spawners(pos: ColumnPos, nbt: &[u8]) -> io::Result<Vec<Spawner>> {
    if nbt.is_empty() {
        return Ok(Vec::new());
    }
    let root = parse(nbt)?;
    if let Some(Value::Compound(level)) = compound(&root)?.get("Level") {
        spawners_in_level(pos, level)
    } else {
        Ok(Vec::new())
    }
}

pub(crate) fn generated_nbt(pos: ColumnPos, col: &Column) -> io::Result<Vec<u8>> {
    encode_column(pos, col, None, Vec::new())
}

#[cfg(test)]
mod spawner_tests {
    use super::*;
    #[test]
    fn nether_chests_and_population_reopen_in_both_quadrants() -> io::Result<()> {
        use crate::overworld::{Dimension, DimensionTerrain, fortress};
        let seed = 12345;
        let provider = DimensionTerrain::new(seed, Dimension::Nether);
        let mut columns = BTreeMap::new();
        for [x, y, z] in [[-67, 60, -489], [516, 53, 274]] {
            let pos = (x >> 4, z >> 4);
            let col = provider.column(pos.0, pos.1);
            let area = fortress::FortressSpawnArea::for_column(seed, pos.0, pos.1);
            assert_eq!(
                area.monsters_at([x, y, z], 0)
                    .iter()
                    .map(|e| (e.entity, e.weight, e.min_group, e.max_group))
                    .collect::<Vec<_>>(),
                [
                    ("Blaze", 10, 2, 3),
                    ("PigZombie", 5, 4, 4),
                    ("Skeleton", 10, 4, 4),
                    ("LavaSlime", 3, 4, 4)
                ]
            );
            assert!(area.monsters_at([x, 255, z], 112 << 4).is_empty());
            let section = &col.sections[&((y >> 4) as u8)];
            assert!(matches!(
                section.state(((y & 15) * 256 + (z & 15) * 16 + (x & 15)) as usize),
                866 | 867
            ));
            columns.insert(pos, col);
        }
        let base = Template {
            spawn_hint: None,
            columns,
        };
        let dir = tempfile::tempdir()?;
        export(dir.path(), &base, &Edits::new())?;
        let loaded = import(dir.path())?;
        for (pos, col) in base.columns {
            let reopened = &loaded.columns[&pos];
            let original = parse(&col.nbt)?;
            let actual = parse(&reopened.nbt)?;
            assert_eq!(original, actual);
            let level = compound(&compound(&actual)?["Level"])?;
            let Value::List(tiles) = &level["TileEntities"] else {
                panic!("missing tiles")
            };
            let chest = tiles
                .iter()
                .filter_map(|v| compound(v).ok())
                .find(|t| t["id"] == Value::String("Chest".into()))
                .unwrap();
            let Value::List(items) = &chest["Items"] else {
                panic!("missing inventory")
            };
            let expected = if pos == (-5, -31) {
                [(10, "saddle", 1), (14, "gold_ingot", 2)]
            } else {
                [(4, "nether_wart", 7), (22, "iron_horse_armor", 1)]
            };
            assert_eq!(items.len(), expected.len());
            for (item, (slot, id, count)) in items.iter().zip(expected) {
                let fields = compound(item)?;
                assert_eq!(fields["Slot"], Value::Byte(slot));
                assert_eq!(fields["id"], Value::String(format!("minecraft:{id}")));
                assert_eq!(fields["Count"], Value::Byte(count));
                assert_eq!(fields["Damage"], Value::Short(0));
            }
            let mut quartz = 0;
            for (sy, section) in col.sections {
                for i in 0..4096 {
                    let state = section.state(i);
                    assert_eq!(state, reopened.sections[&sy].state(i));
                    quartz += usize::from(state == 153 << 4);
                }
            }
            assert!(quartz > 0);
        }
        Ok(())
    }
    #[test]
    fn nether_decorations_and_fluid_ticks_reopen() -> io::Result<()> {
        use crate::overworld::{Dimension, DimensionTerrain};
        let provider = DimensionTerrain::new(12345, Dimension::Nether);
        for sign in [-1, 1] {
            let mut found = std::collections::BTreeSet::new();
            let mut columns = BTreeMap::new();
            let sample = if sign < 0 {
                [1, 2, 4, 5, 8]
            } else {
                [1, 2, 3, 4, 7]
            };
            for z in sample {
                let pos = (sign, z * sign);
                let col = provider.column(pos.0, pos.1);

                for section in col.sections.values() {
                    for &state in &section.palette {
                        if matches!(state >> 4, 39 | 40 | 51 | 89 | 153) {
                            found.insert(state >> 4);
                        }
                    }
                }
                if !col.nbt.is_empty()
                    && compound(&compound(&parse(&col.nbt)?)?["Level"])?.contains_key("TileTicks")
                {
                    found.insert(10);
                }
                columns.insert(pos, col);
            }
            assert_eq!(found.len(), 6);
            let base = Template {
                spawn_hint: None,
                columns,
            };
            let dir = tempfile::tempdir()?;
            export(dir.path(), &base, &Edits::new())?;
            let reopened = import(dir.path())?;
            for (pos, col) in base.columns {
                let loaded = &reopened.columns[&pos];
                for (sy, section) in col.sections {
                    for i in 0..4096 {
                        assert_eq!(section.state(i), loaded.sections[&sy].state(i));
                    }
                }
                let nbt = parse(&loaded.nbt)?;
                let level = compound(&compound(&nbt)?["Level"])?;
                if !col.nbt.is_empty() {
                    let original = parse(&col.nbt)?;
                    assert_eq!(
                        compound(&compound(&original)?["Level"])?.get("TileTicks"),
                        level.get("TileTicks")
                    );
                }
                if let Some(Value::List(ticks)) = level.get("TileTicks") {
                    assert!(!ticks.is_empty());
                    for tick in ticks {
                        let tag = compound(tick)?;
                        let (Value::Int(x), Value::Int(y), Value::Int(z)) =
                            (&tag["x"], &tag["y"], &tag["z"])
                        else {
                            panic!("missing tick position")
                        };
                        assert_eq!((*x >> 4, *z >> 4), pos);
                        assert_eq!(tag["i"], Value::String("minecraft:flowing_lava".into()));
                        assert_eq!(tag["t"], Value::Int(10));
                        assert_eq!(tag["p"], Value::Int(0));
                        let state = loaded.sections[&((y >> 4) as u8)]
                            .state(((y & 15) * 256 + (z & 15) * 16 + (x & 15)) as usize);
                        assert!(matches!(state, 161 | 168));
                    }
                }
            }
        }
        Ok(())
    }
    #[test]
    fn fortress_spawner_survives_region_and_arena_roundtrip() -> io::Result<()> {
        let provider =
            crate::overworld::DimensionTerrain::new(12345, crate::overworld::Dimension::Nether);
        let pos = (365 >> 4, -898 >> 4);
        let base = Template {
            spawn_hint: None,
            columns: BTreeMap::from([(pos, provider.column(pos.0, pos.1))]),
        };
        let dir = tempfile::tempdir()?;
        export(dir.path(), &base, &Edits::new())?;
        let region = import(dir.path())?;
        let file = dir.path().join("test.car");
        region.save(&file)?;
        let arena = Template::load(&file)?;
        for source in [&base, &region, &arena] {
            let tiles = &source.columns[&pos].spawners;
            assert_eq!(tiles.len(), 1);
            assert_eq!(tiles[0].pos, (365, 58, -898));
            let tag = parse(&tiles[0].data)?;
            let fields = compound(&tag)?;
            assert_eq!(fields["EntityId"], Value::String("Blaze".into()));
            assert_eq!(fields["SpawnRange"], Value::Short(4));
            assert_eq!(fields["Delay"], Value::Short(20));
        }
        let edits = Edits::from([(pos, BTreeMap::from([((365, 58, -898), 0)]))]);
        export(dir.path(), &region, &edits)?;
        assert!(import(dir.path())?.columns[&pos].spawners.is_empty());
        Ok(())
    }
}
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
        spawners: spawners_in_level(pos, level)?,
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
    generated_entities: Vec<Value>,
) -> io::Result<Vec<u8>> {
    if changes.is_none_or(|c| c.is_empty()) && !col.nbt.is_empty() {
        return Ok(col.nbt.clone());
    }
    let mut root = if col.nbt.is_empty() {
        Compound::new()
    } else {
        compound(&parse(&col.nbt)?)?.clone()
    };
    let level = match root.remove("Level") {
        Some(Value::Compound(v)) => v,
        _ => Compound::new(),
    };
    encode_level(pos, col, changes, generated_entities, root, level)
}

pub(crate) fn store_overworld_tiles(
    pos: ColumnPos,
    col: &mut Column,
    tiles: Vec<Value>,
    ticks: Vec<Value>,
) -> io::Result<()> {
    if tiles.is_empty() && ticks.is_empty() {
        return Ok(());
    }
    let level = Compound::from([
        ("TileEntities".into(), Value::List(tiles)),
        ("TileTicks".into(), Value::List(ticks)),
    ]);
    col.spawners = spawners_in_level(pos, &level)?;
    col.nbt = encode_level(pos, col, None, Vec::new(), Compound::new(), level)?;
    Ok(())
}

pub(crate) fn store_nether_features(
    pos: ColumnPos,
    col: &mut Column,
    mut chests: Vec<Value>,
    ticks: BTreeMap<[i32; 3], i32>,
) {
    if chests.is_empty() && ticks.is_empty() {
        return;
    }
    chests.extend(
        col.spawners
            .iter()
            .map(|s| parse(&s.data).expect("generated spawner NBT")),
    );
    let mut level = Compound::from([("TileEntities".into(), Value::List(chests))]);
    if !ticks.is_empty() {
        let tags = ticks
            .into_iter()
            .map(|([x, y, z], delay)| {
                Value::Compound(
                    [
                        ("i".into(), Value::String("minecraft:flowing_lava".into())),
                        ("t".into(), Value::Int(delay)),
                        ("p".into(), Value::Int(0)),
                        ("x".into(), Value::Int(x)),
                        ("y".into(), Value::Int(y)),
                        ("z".into(), Value::Int(z)),
                    ]
                    .into(),
                )
            })
            .collect();
        level.insert("TileTicks".into(), Value::List(tags));
    }
    col.nbt = encode_level(pos, col, None, Vec::new(), Compound::new(), level)
        .expect("generated Nether column");
}

fn encode_level(
    pos: ColumnPos,
    col: &Column,
    changes: Option<&BTreeMap<BlockPos, u16>>,
    generated_entities: Vec<Value>,
    mut root: Compound,
    mut level: Compound,
) -> io::Result<Vec<u8>> {
    level.insert("xPos".into(), Value::Int(pos.0));
    level.insert("zPos".into(), Value::Int(pos.1));
    level.insert("Biomes".into(), array(&col.biomes));
    level
        .entry("Entities".into())
        .or_insert(Value::List(generated_entities));
    if !level.contains_key("TileEntities") {
        let tags = col
            .spawners
            .iter()
            .map(|s| parse(&s.data))
            .collect::<io::Result<Vec<_>>>()?;
        level.insert("TileEntities".into(), Value::List(tags));
    }
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

pub(crate) fn store_end_crystals(
    template: &mut Template,
    provider: &crate::overworld::DimensionTerrain,
    seed: i64,
) -> io::Result<()> {
    let mut groups = BTreeMap::<ColumnPos, Vec<Value>>::new();
    for spike in provider.spikes() {
        let [x, y, z] = spike.bedrock();
        use md5::{Digest, Md5};
        let digest = Md5::digest(format!("EnderCrystal/{seed}/{x}/{y}/{z}").as_bytes());
        let uuid = uuid::Builder::from_md5_bytes(digest.into())
            .into_uuid()
            .as_u128();
        let fields = [
            ("id", Value::String("EnderCrystal".into())),
            (
                "Pos",
                Value::List(spike.crystal().map(Value::Double).to_vec()),
            ),
            ("Motion", Value::List(vec![Value::Double(0.); 3])),
            (
                "Rotation",
                Value::List(vec![Value::Float(spike.crystal_yaw), Value::Float(0.)]),
            ),
            ("UUIDMost", Value::Long((uuid >> 64) as i64)),
            ("UUIDLeast", Value::Long(uuid as i64)),
            ("Air", Value::Short(300)),
            ("Dimension", Value::Int(1)),
            ("Fire", Value::Short(0)),
            ("FallDistance", Value::Float(0.)),
            ("OnGround", Value::Byte(0)),
            ("PortalCooldown", Value::Int(0)),
            ("Invulnerable", Value::Byte(0)),
        ];
        groups
            .entry((x >> 4, z >> 4))
            .or_default()
            .push(Value::Compound(
                fields.into_iter().map(|(k, v)| (k.into(), v)).collect(),
            ));
    }
    for (pos, entities) in groups {
        let col = template
            .columns
            .entry(pos)
            .or_insert_with(|| provider.column(pos.0, pos.1));
        col.nbt = encode_column(pos, col, None, entities)?;
    }
    Ok(())
}

pub(crate) fn load_end_crystals(
    template: &Template,
    entities: &mut crate::entities::EntityArena,
) -> io::Result<()> {
    #[derive(serde::Deserialize)]
    struct Root {
        #[serde(rename = "Level")]
        level: Level,
    }
    #[derive(serde::Deserialize)]
    struct Level {
        #[serde(rename = "Entities", default)]
        entities: Vec<SavedEntity>,
    }
    #[derive(serde::Deserialize)]
    struct SavedEntity {
        id: String,
        #[serde(rename = "Pos")]
        pos: [f64; 3],
        #[serde(rename = "Rotation")]
        rotation: [f32; 2],
    }
    for col in template.columns.values().filter(|c| !c.nbt.is_empty()) {
        let root: Root = fastnbt::from_bytes(&col.nbt).map_err(io::Error::other)?;
        for e in root.level.entities {
            if e.id == "EnderCrystal" {
                entities
                    .insert(crate::entities::Entity {
                        kind: crate::entities::EntityKind::EndCrystal {
                            yaw: (e.rotation[0] * 256. / 360.) as i32 as u8,
                        },
                        position: e.pos,
                        velocity: [0.; 3],
                        age: 0,
                    })
                    .map_err(|_| invalid("End crystals exceed entity capacity"))?;
            }
        }
    }
    Ok(())
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
            let raw = encode_column(pos, col, edits.get(&pos), Vec::new())?;
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

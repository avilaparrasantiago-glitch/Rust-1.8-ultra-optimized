use super::{Compound, invalid};
use fastnbt::Value;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::{Serialize, Serializer};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Village,
    Stronghold,
    Mineshaft,
    Temple,
    Monument,
}

impl Family {
    fn name(self) -> &'static str {
        match self {
            Self::Village => "Village",
            Self::Stronghold => "Stronghold",
            Self::Mineshaft => "Mineshaft",
            Self::Temple => "Temple",
            Self::Monument => "Monument",
        }
    }
}

struct Ordered<'a>(&'a Value);

impl Serialize for Ordered<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Compound(fields) => serializer.collect_map(
                fields
                    .iter()
                    .map(|(key, value)| (key, Ordered(value)))
                    .collect::<BTreeMap<_, _>>(),
            ),
            Value::List(values) => serializer.collect_seq(values.iter().map(Ordered)),
            value => value.serialize(serializer),
        }
    }
}

#[derive(Clone)]
pub struct StructureFile {
    family: Family,
    features: BTreeMap<(i32, i32), Compound>,
}

impl StructureFile {
    pub(crate) fn empty(family: Family) -> Self {
        Self {
            family,
            features: BTreeMap::new(),
        }
    }
    fn path(&self, world: &Path) -> PathBuf {
        world
            .join("data")
            .join(format!("{}.dat", self.family.name()))
    }

    pub fn load(world: &Path, family: Family) -> io::Result<Self> {
        let mut result = Self {
            family,
            features: BTreeMap::new(),
        };
        let file = match File::open(result.path(world)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(result),
            Err(error) => return Err(error),
        };
        let mut raw = Vec::new();
        const LIMIT: u64 = 64 << 20;
        GzDecoder::new(file).take(LIMIT + 1).read_to_end(&mut raw)?;
        if raw.len() as u64 > LIMIT {
            return Err(invalid("oversized structure file"));
        }
        let mut root: Compound = fastnbt::from_bytes(&raw).map_err(io::Error::other)?;
        let Some(Value::Compound(mut data)) = root.remove("data") else {
            return Err(invalid("missing structure data"));
        };
        let Some(Value::Compound(features)) = data.remove("Features") else {
            return Err(invalid("missing structure Features"));
        };
        for (key, value) in features {
            let Value::Compound(start) = value else {
                return Err(invalid("invalid structure start"));
            };
            let (x, z) = result.coordinates(&start)?;
            if key != format!("[{x},{z}]") {
                return Err(invalid("structure start key mismatch"));
            }
            result.features.insert((x, z), start);
        }
        Ok(result)
    }

    fn coordinates(&self, start: &Compound) -> io::Result<(i32, i32)> {
        match (start.get("ChunkX"), start.get("ChunkZ"), start.get("id")) {
            (Some(Value::Int(x)), Some(Value::Int(z)), Some(Value::String(id)))
                if id == self.family.name() =>
            {
                Ok((*x, *z))
            }
            _ => Err(invalid("invalid structure start identity")),
        }
    }

    pub fn insert(&mut self, start: Compound) -> io::Result<()> {
        self.features.insert(self.coordinates(&start)?, start);
        Ok(())
    }

    pub(crate) fn record_pyramid_chests(&mut self, placed: &[[i32; 3]]) -> io::Result<()> {
        for &[x, z, flag] in placed {
            let piece = self
                .features
                .get_mut(&(x, z))
                .and_then(|start| start.get_mut("Children"));
            let Some(Value::List(children)) = piece else {
                return Err(invalid("missing pyramid"));
            };
            let Some(Value::Compound(child)) = children.first_mut() else {
                return Err(invalid("missing pyramid piece"));
            };
            if !(0..4).contains(&flag)
                || !matches!(child.get("id"),Some(Value::String(id)) if id=="TeDP")
            {
                return Err(invalid("invalid pyramid chest"));
            }
            child.insert(format!("hasPlacedChest{flag}"), Value::Byte(1));
        }
        Ok(())
    }

    /// Swamp huts fit inside their start chunk, including after rotation.
    pub fn witch_spawn_at(&self, [x, y, z]: [i32; 3]) -> bool {
        if self.family != Family::Temple {
            return false;
        }
        let Some(Value::List(children)) = self.get(x >> 4, z >> 4).and_then(|s| s.get("Children"))
        else {
            return false;
        };
        children.iter().any(|child| {
            let Value::Compound(fields)=child else {return false;};
            matches!(fields.get("id"),Some(Value::String(id)) if id=="TeSH")
                && matches!(fields.get("BB"),Some(Value::IntArray(b)) if b.len()==6
                && (b[0]..=b[3]).contains(&x) && (b[1]..=b[4]).contains(&y) && (b[2]..=b[5]).contains(&z))
        })
    }

    pub fn get(&self, x: i32, z: i32) -> Option<&Compound> {
        self.features.get(&(x, z))
    }

    pub(crate) fn reopened_eye_targets(&self) -> io::Result<Vec<[i32; 3]>> {
        if self.family != Family::Stronghold {
            return Err(invalid("expected Stronghold.dat"));
        }
        self.features
            .values()
            .map(|start| {
                let Some(Value::List(children)) = start.get("Children") else {
                    return Err(invalid("missing stronghold children"));
                };
                let Some(Value::Compound(stairs)) = children.first() else {
                    return Err(invalid("missing stronghold start stairs"));
                };
                let Some(Value::IntArray(bounds)) = stairs.get("BB") else {
                    return Err(invalid("missing stronghold start bounds"));
                };
                if bounds.len() != 6 || (0..3).any(|axis| bounds[axis] > bounds[axis + 3]) {
                    return Err(invalid("invalid stronghold start bounds"));
                }
                Ok(std::array::from_fn(|axis| {
                    let low = i64::from(bounds[axis]);
                    (low + (i64::from(bounds[axis + 3]) - low + 1) / 2) as i32
                }))
            })
            .collect()
    }

    pub fn save(&self, world: &Path) -> io::Result<()> {
        let features = self
            .features
            .iter()
            .map(|((x, z), fields)| {
                (
                    format!("[{x},{z}]"),
                    fields
                        .iter()
                        .map(|(key, value)| (key, Ordered(value)))
                        .collect::<BTreeMap<_, _>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let root = BTreeMap::from([("data", BTreeMap::from([("Features", features)]))]);
        let directory = world.join("data");
        fs::create_dir_all(&directory)?;
        let mut temp = tempfile::NamedTempFile::new_in(directory)?;
        let mut zip = GzEncoder::new(temp.as_file_mut(), Compression::fast());
        fastnbt::to_writer(&mut zip, &root).map_err(io::Error::other)?;
        zip.finish()?.flush()?;
        temp.persist(self.path(world))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portal_reference_is_not_restored_by_the_vanilla_loader() -> io::Result<()> {
        let world = tempfile::tempdir()?;
        let mut index = StructureFile::load(world.path(), Family::Stronghold)?;
        let start = crate::overworld::Overworld::new(12345)
            .stronghold_start_nbt(-44, 59)
            .unwrap();
        index.insert(start)?;
        index.save(world.path())?;
        assert_eq!(
            StructureFile::load(world.path(), Family::Stronghold)?.reopened_eye_targets()?,
            [[-700, 43, 948]]
        );
        Ok(())
    }
    use std::collections::HashMap;

    #[test]
    fn starts_across_region_edges_survive_reopening() -> io::Result<()> {
        let world = tempfile::tempdir()?;
        for family in [Family::Stronghold, Family::Mineshaft] {
            let child = HashMap::from([
                (
                    "id".into(),
                    Value::String(
                        if family == Family::Mineshaft {
                            "MSCorridor"
                        } else {
                            "SHLibrary"
                        }
                        .into(),
                    ),
                ),
                (
                    "BB".into(),
                    Value::IntArray(fastnbt::IntArray::new(vec![-7, 20, 505, 7, 25, 521])),
                ),
            ]);
            let start = HashMap::from([
                ("id".into(), Value::String(family.name().into())),
                ("ChunkX".into(), Value::Int(-1)),
                ("ChunkZ".into(), Value::Int(31)),
                ("Children".into(), Value::List(vec![Value::Compound(child)])),
            ]);
            let mut starts = StructureFile::load(world.path(), family)?;
            starts.insert(start.clone())?;
            starts.save(world.path())?;
            let bytes = std::fs::read(starts.path(world.path()))?;
            let mut reopened = StructureFile::load(world.path(), family)?;
            assert_eq!(reopened.get(-1, 31), Some(&start));
            reopened.insert(start)?;
            reopened.save(world.path())?;
            assert_eq!(bytes, std::fs::read(reopened.path(world.path()))?);
        }
        Ok(())
    }
}

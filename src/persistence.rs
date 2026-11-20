use crate::{
    anvil::{self, Edits},
    arena::Template,
};
use crossbeam_channel::{Sender, TrySendError};
use std::{fs::File, io, path::Path, sync::Arc, thread::JoinHandle};

enum Save {
    Snapshot(Arc<Edits>),
    Stop(Arc<Edits>),
}

type Opened = (Store, Arc<Template>, Arc<Edits>);
pub struct Store {
    tx: Sender<Save>,
    worker: Option<JoinHandle<io::Result<()>>>,
}
impl Store {
    pub fn open(dir: &Path, base: Arc<Template>, seed: Option<i64>) -> io::Result<Opened> {
        std::fs::create_dir_all(dir)?;
        let lease = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(".coral-owner"))?;
        lease.try_lock().map_err(io::Error::other)?;
        let normal = seed.is_some();
        let marker = dir.join("generator.json");
        if marker.try_exists()? {
            let saved: GeneratorIdentity = serde_json::from_reader(File::open(&marker)?)?;
            if Some(saved) != seed.map(GeneratorIdentity::legacy) {
                return Err(crate::wire::invalid(
                    "generator identity differs from saved world",
                ));
            }
        }
        let exists = std::fs::read_dir(dir)?
            .filter_map(Result::ok)
            .any(|e| e.path().extension().is_some_and(|x| x == "mca"));
        let template = if exists {
            Arc::new(anvil::import(dir)?)
        } else {
            base
        };
        template.spawn()?;
        let deltas = dir.join("generated.edits");
        let edits = if normal && deltas.try_exists()? {
            read_deltas(&deltas)?
        } else {
            Edits::new()
        };
        if let Some(seed) = seed
            && !marker.try_exists()?
        {
            let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
            serde_json::to_writer(&mut tmp, &GeneratorIdentity::legacy(seed))?;
            tmp.as_file().sync_all()?;
            tmp.persist_noclobber(marker).map_err(|e| e.error)?;
        }
        let world = template.clone();
        let dir = dir.to_owned();
        let (tx, rx) = crossbeam_channel::bounded(1);
        let worker = std::thread::Builder::new()
            .name("anvil-writer".into())
            .spawn(move || {
                let _owner: File = lease;
                let save = |s: &Edits| -> io::Result<()> {
                    anvil::export(&dir, &world, s)?;
                    if normal {
                        write_deltas(&dir, &deltas, &world, s)?;
                    }
                    Ok(())
                };
                while let Ok(event) = rx.recv() {
                    match event {
                        Save::Snapshot(s) => {
                            if let Err(e) = save(&s) {
                                eprintln!("Anvil save failed for {}: {e}", dir.display());
                            }
                        }
                        Save::Stop(s) => return save(&s),
                    }
                }
                Ok(())
            })?;
        Ok((
            Self {
                tx,
                worker: Some(worker),
            },
            template,
            Arc::new(edits),
        ))
    }
    pub fn save(&self, edits: Arc<Edits>) -> bool {
        match self.tx.try_send(Save::Snapshot(edits)) {
            Ok(()) => true,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
        }
    }
    pub fn close(&mut self, edits: Arc<Edits>) -> io::Result<()> {
        self.tx.send(Save::Stop(edits)).map_err(io::Error::other)?;
        self.worker
            .take()
            .unwrap()
            .join()
            .map_err(|_| io::Error::other("Anvil worker panicked"))?
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct GeneratorIdentity {
    dimension: i32,
    algorithm: String,
    revision: u32,
    options: String,
    seed: i64,
}

impl GeneratorIdentity {
    fn legacy(seed: i64) -> Self {
        Self {
            dimension: 0,
            algorithm: "syrust-overworld".into(),
            revision: 1,
            options: String::new(),
            seed,
        }
    }
}

/// Sparse overrides outside the fixed Anvil base. Untouched columns are
/// reproducible from the world seed and never need a disk record.
fn write_deltas(dir: &Path, path: &Path, base: &Template, edits: &Edits) -> io::Result<()> {
    use io::Write;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    {
        let mut out = io::BufWriter::new(&mut file);
        out.write_all(b"OWED1")?;
        for (col, entries) in edits {
            if base.contains(*col) {
                continue;
            }
            for (&(x, y, z), &state) in entries {
                out.write_all(&x.to_le_bytes())?;
                out.write_all(&z.to_le_bytes())?;
                out.write_all(&[y])?;
                out.write_all(&state.to_le_bytes())?;
            }
        }
        out.flush()?;
    }
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}

fn read_deltas(path: &Path) -> io::Result<Edits> {
    use io::Read;
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut input = io::BufReader::new(file);
    let mut magic = [0u8; 5];
    input.read_exact(&mut magic)?;
    if &magic != b"OWED1" || !(size - 5).is_multiple_of(11) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid generated edits",
        ));
    }
    let mut edits = Edits::new();
    for _ in 0..(size - 5) / 11 {
        let mut x = [0; 4];
        let mut z = [0; 4];
        let mut y = [0];
        let mut state = [0; 2];
        input.read_exact(&mut x)?;
        input.read_exact(&mut z)?;
        input.read_exact(&mut y)?;
        input.read_exact(&mut state)?;
        let (x, z) = (i32::from_le_bytes(x), i32::from_le_bytes(z));
        edits
            .entry((x >> 4, z >> 4))
            .or_default()
            .insert((x, y[0], z), u16::from_le_bytes(state));
    }
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adoption_preserves_legacy_deltas_and_rejects_another_seed() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let base = Arc::new(Template::flat(1));
        let mut edits = Edits::new();
        edits
            .entry((-10, 18))
            .or_default()
            .insert((-160, 72, 288), 17);
        let path = dir.path().join("generated.edits");
        write_deltas(dir.path(), &path, &base, &edits)?;
        let original = std::fs::read(&path)?;
        let (mut store, _, loaded) = Store::open(dir.path(), base.clone(), Some(-72))?;
        assert_eq!(*loaded, edits);
        store.close(loaded)?;
        assert_eq!(std::fs::read(&path)?, original);
        assert!(Store::open(dir.path(), base.clone(), Some(42)).is_err());
        let (mut store, _, reopened) = Store::open(dir.path(), base, Some(-72))?;
        assert_eq!(*reopened, edits);
        store.close(reopened)
    }
}

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
    pub fn open(dir: &Path, base: Arc<Template>, normal: bool) -> io::Result<Opened> {
        std::fs::create_dir_all(dir)?;
        let lease = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(".coral-owner"))?;
        lease.try_lock().map_err(io::Error::other)?;
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

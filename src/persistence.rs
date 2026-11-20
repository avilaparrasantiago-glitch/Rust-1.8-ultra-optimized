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

pub struct Store {
    tx: Sender<Save>,
    worker: Option<JoinHandle<io::Result<()>>>,
}
impl Store {
    pub fn open(dir: &Path, base: Arc<Template>) -> io::Result<(Self, Arc<Template>)> {
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
        let world = template.clone();
        let dir = dir.to_owned();
        let (tx, rx) = crossbeam_channel::bounded(1);
        let worker = std::thread::Builder::new()
            .name("anvil-writer".into())
            .spawn(move || {
                let _owner: File = lease;
                while let Ok(event) = rx.recv() {
                    match event {
                        Save::Snapshot(s) => {
                            if let Err(e) = anvil::export(&dir, &world, &s) {
                                eprintln!("Anvil save failed for {}: {e}", dir.display());
                            }
                        }
                        Save::Stop(s) => return anvil::export(&dir, &world, &s),
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

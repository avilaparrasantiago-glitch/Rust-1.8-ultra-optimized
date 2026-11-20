use crate::{packets::Stack, wire::invalid};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use sqlx::{AnyPool, Row, any::AnyPoolOptions};
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub slots: Vec<Option<Stack>>,
    pub cursor: Option<Stack>,
    pub table: Vec<Option<Stack>>,
    pub xp: i32,
    pub age: u64,
    pub deaths: i32,
    pub kills: i32,
    pub team: String,
    pub scores: HashMap<String, i32>,
}
impl Profile {
    pub fn validate(&self) -> io::Result<()> {
        if (!self.slots.is_empty() && self.slots.len() != 45)
            || (!self.table.is_empty() && self.table.len() != 9)
            || self.xp < 0
            || self.age > i64::MAX as u64
            || self.scores.len() > 4096
            || self
                .slots
                .iter()
                .chain(&self.table)
                .flatten()
                .chain(self.cursor.iter())
                .any(|s| s.id <= 0 || s.count == 0 || s.count > 64)
        {
            return Err(invalid("invalid saved profile"));
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
struct Write {
    uuid: Uuid,
    owner: Uuid,
    seq: i64,
    end: bool,
    data: Profile,
}
enum Message {
    Save(Write),
    Shutdown(oneshot::Sender<io::Result<()>>),
}
pub struct Profiles {
    pool: AnyPool,
    tx: mpsc::Sender<Message>,
}
pub struct Lease {
    uuid: Uuid,
    owner: Uuid,
    seq: i64,
    tx: mpsc::Sender<Message>,
    reserved: Option<mpsc::OwnedPermit<Message>>,
}
impl Lease {
    pub fn checkpoint(&mut self, data: Profile) {
        self.seq += 1;
        let _ = self.tx.try_send(Message::Save(Write {
            uuid: self.uuid,
            owner: self.owner,
            seq: self.seq,
            end: false,
            data,
        }));
    }
    pub fn finish(mut self, data: Profile) {
        self.seq += 1;
        self.reserved.take().unwrap().send(Message::Save(Write {
            uuid: self.uuid,
            owner: self.owner,
            seq: self.seq,
            end: true,
            data,
        }));
    }
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
impl Profiles {
    pub async fn open(dir: &Path, url: Option<String>, capacity: usize) -> io::Result<Arc<Self>> {
        sqlx::any::install_default_drivers();
        let url = match url {
            Some(url) => url,
            None => {
                let path = dir.join("profiles.sqlite");
                let path = path
                    .to_str()
                    .ok_or_else(|| invalid("profile database path must be valid UTF-8"))?;
                // An opaque SQLite URL keeps the filename out of the URL authority.
                format!(
                    "sqlite:{}?mode=rwc",
                    utf8_percent_encode(path, NON_ALPHANUMERIC)
                )
            }
        };
        let pool = AnyPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&url)
            .await
            .map_err(io::Error::other)?;
        sqlx::query("CREATE TABLE IF NOT EXISTS coral_profiles (uuid TEXT PRIMARY KEY, owner TEXT NOT NULL, expires BIGINT NOT NULL, seq BIGINT NOT NULL, data TEXT NOT NULL)")
            .execute(&pool).await.map_err(io::Error::other)?;
        let spool = dir.join("profile-recovery");
        tokio::fs::create_dir_all(&spool).await?;
        let mut files = tokio::fs::read_dir(&spool).await?;
        while let Some(entry) = files.next_entry().await? {
            if entry.path().extension().is_some_and(|e| e == "json") {
                let write: Write = serde_json::from_slice(&tokio::fs::read(entry.path()).await?)?;
                persist(&pool, &write).await?;
                tokio::fs::remove_file(entry.path()).await?;
            }
        }
        let (tx, rx) = mpsc::channel(capacity + 1024);
        tokio::spawn(writer(pool.clone(), rx, spool));
        Ok(Arc::new(Self { pool, tx }))
    }
    pub async fn claim(&self, uuid: Uuid) -> io::Result<(Lease, Profile)> {
        let reserved = self
            .tx
            .clone()
            .reserve_owned()
            .await
            .map_err(io::Error::other)?;
        let owner = Uuid::new_v4();
        // The prior session owns the row until its final write or its lease expiry.
        for _ in 0..20 {
            let ts = now();
            let row = sqlx::query(
                "INSERT INTO coral_profiles(uuid,owner,expires,seq,data) VALUES($1,$2,$3,0,'{}')
              ON CONFLICT(uuid) DO UPDATE SET owner=excluded.owner, expires=excluded.expires, seq=0
              WHERE coral_profiles.owner='' OR coral_profiles.expires<$4 RETURNING data",
            )
            .bind(uuid.to_string())
            .bind(owner.to_string())
            .bind(ts + 90)
            .bind(ts)
            .fetch_optional(&self.pool)
            .await
            .map_err(io::Error::other)?;
            if let Some(row) = row {
                let profile: Profile = serde_json::from_str(
                    row.try_get::<&str, _>("data").map_err(io::Error::other)?,
                )?;
                profile.validate()?;
                return Ok((
                    Lease {
                        uuid,
                        owner,
                        seq: 0,
                        tx: self.tx.clone(),
                        reserved: Some(reserved),
                    },
                    profile,
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err(invalid("profile is active on another session"))
    }
    pub async fn close(&self) -> io::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::Shutdown(tx))
            .await
            .map_err(io::Error::other)?;
        rx.await.map_err(io::Error::other)??;
        self.pool.close().await;
        Ok(())
    }
}
async fn persist(pool: &AnyPool, w: &Write) -> io::Result<()> {
    sqlx::query("UPDATE coral_profiles SET owner=$1,expires=$2,seq=$3,data=$4 WHERE uuid=$5 AND owner=$6 AND seq<$3")
        .bind(if w.end{String::new()}else{w.owner.to_string()}).bind(if w.end{0}else{now()+90}).bind(w.seq)
        .bind(serde_json::to_string(&w.data)?).bind(w.uuid.to_string()).bind(w.owner.to_string()).execute(pool).await.map_err(io::Error::other)?;
    Ok(())
}
async fn writer(pool: AnyPool, mut rx: mpsc::Receiver<Message>, spool: PathBuf) {
    while let Some(first) = rx.recv().await {
        let mut pending = HashMap::new();
        let mut stop = None;
        let mut msg = Some(first);
        for _ in 0..64 {
            match msg.take().or_else(|| rx.try_recv().ok()) {
                Some(Message::Save(w)) => {
                    pending.insert((w.uuid, w.owner), w);
                }
                Some(Message::Shutdown(ack)) => {
                    stop = Some(ack);
                    break;
                }
                None => break,
            }
        }
        let mut result = Ok(());
        for (_, w) in pending {
            if let Err(e) = persist(&pool, &w).await {
                eprintln!("Profile database unavailable, journaling checkpoint: {e}");
                let dir = spool.clone();
                let recovery = tokio::task::spawn_blocking(move || -> io::Result<()> {
                    let mut f = tempfile::NamedTempFile::new_in(&dir)?;
                    serde_json::to_writer(&mut f, &w)?;
                    f.as_file().sync_all()?;
                    f.persist(dir.join(format!("{}-{}-{}.json", w.uuid, w.owner, w.seq)))
                        .map_err(|e| e.error)?;
                    Ok(())
                })
                .await;
                if let Err(e) = recovery.unwrap_or_else(|e| Err(io::Error::other(e))) {
                    eprintln!("Profile journal failure: {e}");
                    result = Err(e);
                }
            }
        }
        if let Some(tx) = stop {
            let _ = tx.send(result);
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Profiles;

    #[tokio::test]
    async fn opens_default_database_in_directory_with_url_characters() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("world # 100%");
        std::fs::create_dir(&dir).unwrap();

        let profiles = Profiles::open(&dir, None, 1).await.unwrap();
        profiles.close().await.unwrap();

        assert!(dir.join("profiles.sqlite").is_file());
    }
}

use crate::wire::invalid;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io,
    net::SocketAddr,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Terrain {
    #[default]
    Flat,
    Normal,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub listen: SocketAddr,
    pub metrics: SocketAddr,
    pub max_players: usize,
    pub view_distance: u8,
    pub world_distances: BTreeMap<String, WorldDistances>,
    pub worlds: Vec<String>,
    pub dimensions: BTreeMap<String, crate::overworld::Dimension>,
    pub persistent_worlds: Vec<String>,
    pub scripts: BTreeMap<String, PathBuf>,
    #[serde(default)] // Older config files describe flat worlds.
    pub terrain: Terrain,
    pub arena_file: Option<PathBuf>,
    pub data_dir: PathBuf,
    pub motd: String,
    pub operators: BTreeMap<Uuid, u8>,
    pub permissions: BTreeMap<Uuid, Vec<String>>,
    pub database_url_env: Option<String>,
    pub redis_url_env: Option<String>,
    pub nats_url_env: Option<String>,
    pub proxy_secret_env: Option<String>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorldDistances {
    pub simulation_distance: u8,
    pub horizon_view_distance: u8,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:25565".parse().unwrap(),
            metrics: "127.0.0.1:9900".parse().unwrap(),
            dimensions: BTreeMap::new(),
            persistent_worlds: Vec::new(),
            scripts: BTreeMap::new(),
            max_players: 128,
            view_distance: 3,
            world_distances: BTreeMap::new(),
            worlds: vec!["lobby".into(), "arena".into()],
            terrain: Terrain::Normal,
            arena_file: None,
            data_dir: "data".into(),
            motd: "SyRust · Protocol 47".into(),
            operators: BTreeMap::new(),
            permissions: BTreeMap::new(),
            database_url_env: None,
            redis_url_env: None,
            nats_url_env: None,
            proxy_secret_env: None,
        }
    }
}
impl Config {
    pub fn distances(&self, world: &str) -> WorldDistances {
        self.world_distances
            .get(world)
            .copied()
            .unwrap_or(WorldDistances {
                simulation_distance: self.view_distance,
                horizon_view_distance: self.view_distance,
            })
    }

    pub fn load(file: Option<&Path>) -> io::Result<Self> {
        let c: Self = match file {
            Some(p) => serde_json::from_slice(&std::fs::read(p)?).map_err(io::Error::other)?,
            None => Self::default(),
        };
        if !c.listen.ip().is_loopback() && c.proxy_secret_env.is_none() {
            return Err(invalid("public listeners require signed proxy forwarding"));
        }
        if c.dimensions.keys().any(|n| {
            !c.worlds.contains(n) || c.arena_file.is_some() || !matches!(c.terrain, Terrain::Normal)
        }) || c.world_distances.iter().any(|(world, d)| {
            !c.worlds.contains(world)
                || !(8..=12).contains(&d.simulation_distance)
                || !(d.simulation_distance..=64).contains(&d.horizon_view_distance)
        }) || c.persistent_worlds.iter().any(|n| !c.worlds.contains(n))
            || !(1..=8).contains(&c.view_distance)
            || !(1..=1_000_000).contains(&c.max_players)
            || c.worlds.is_empty()
            || c.worlds.len() > 128
            || c.operators.values().any(|&l| l > 4)
            || c.worlds.iter().any(|n| !valid_world(n))
            || c.worlds
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != c.worlds.len()
        {
            return Err(invalid(
                "invalid world, distance, player limit or permission level",
            ));
        }
        Ok(c)
    }
}
pub fn valid_world(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
pub fn persistent_seed(dir: &Path) -> io::Result<i64> {
    use std::io::Write;
    std::fs::create_dir_all(dir)?;
    let path = dir.join("seed");
    if !path.exists() {
        let tmp = dir.join(format!("seed-{}.tmp", Uuid::new_v4()));
        let seed = Uuid::new_v4().as_u128() as i64;
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp)?;
        let result = (|| {
            write!(f, "{seed}")?;
            f.sync_all()?;
            drop(f);
            std::fs::hard_link(&tmp, &path)
        })();
        std::fs::remove_file(tmp)?;
        if let Err(e) = result
            && e.kind() != io::ErrorKind::AlreadyExists
        {
            return Err(e);
        }
    }
    std::fs::read_to_string(path)?
        .trim()
        .parse()
        .map_err(|_| invalid("invalid persisted seed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_defaults_preserve_existing_configurations() {
        assert!(matches!(Config::default().terrain, Terrain::Normal));
        assert!(matches!(
            serde_json::from_str::<Config>("{}").unwrap().terrain,
            Terrain::Flat
        ));
    }
}

mod drops;
mod entity_visibility;
mod windows;
use crate::{
    arena::{BlockPos, ColumnPos, Template, World, column},
    config::{Config, valid_world},
    entities::EntityArena,
    identity::Identity,
    input::Action,
    inventory::Inventory,
    metrics::Metrics,
    packets::{self as p, Stack},
    wire::{Packet, WriteWire, invalid},
};
use bytes::BufMut;
use crossbeam_channel::{Receiver, Sender, TryRecvError};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io,
    sync::{
        Arc, RwLock,
        atomic::{
            AtomicBool, AtomicI32, AtomicUsize,
            Ordering::{Acquire, Relaxed, Release},
        },
    },
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

const ENTITIES_PER_ROOM: usize = 1024;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Window {
    Table(BlockPos),
    Furnace(BlockPos),
}
pub struct Player {
    pub lease: Option<crate::profiles::Lease>,
    pub id: i32,
    pub identity: Identity,
    pub pos: [f64; 3],
    pub look: [f32; 2],
    pub mode: u8,
    pub hp: f32,
    pub xp: i32,
    pub scores: HashMap<String, i32>,
    pub team: String,
    pub inv: Inventory,
    pub input: Receiver<Action>,
    output: mpsc::Sender<Packet>,
    failed: AtomicBool,
    pub ground: bool,
    pub flying: bool,
    sprint: bool,
    fall: f64,
    pub spawn: [f64; 3],
    pending: Option<([f64; 3], u64)>,
    pub(crate) window: Option<Window>,
    table: [Option<Stack>; 9],
    dig: Option<(BlockPos, u64, u16)>,
    pub age: u64,
    keep: Option<(i32, u64)>,
    last_hit: u64,
    credit: u32,
    loaded: HashSet<ColumnPos>,
    pending_cols: Vec<ColumnPos>,
    center: Option<ColumnPos>,
    seen: HashMap<i32, u64>,
    seen_entities: entity_visibility::Seen,
    revision: u64,
    moved_tick: f64,
    previous: [f64; 3],
    pub kills: i32,
    pub deaths: i32,
}
impl Player {
    pub fn new(
        id: i32,
        identity: Identity,
        input: Receiver<Action>,
        output: mpsc::Sender<Packet>,
    ) -> Self {
        Self {
            lease: None,
            id,
            identity,
            input,
            output,
            failed: AtomicBool::new(false),
            pos: [0.; 3],
            look: [0.; 2],
            mode: 0,
            hp: 20.,
            xp: 0,
            scores: HashMap::new(),
            team: String::new(),
            inv: Inventory::default(),
            window: None,
            table: [None; 9],
            dig: None,
            ground: false,
            flying: false,
            sprint: false,
            fall: 0.,
            spawn: [0.; 3],
            pending: None,
            age: 0,
            keep: None,
            last_hit: 0,
            credit: 200,
            loaded: HashSet::new(),
            pending_cols: Vec::new(),
            center: None,
            seen: HashMap::new(),
            seen_entities: entity_visibility::empty_seen(),
            revision: 1,
            moved_tick: 0.,
            previous: [0.; 3],
            kills: 0,
            deaths: 0,
        }
    }
    pub fn restore(&mut self, lease: crate::profiles::Lease, profile: crate::profiles::Profile) {
        self.lease = Some(lease);
        self.xp = profile.xp;
        self.age = profile.age;
        self.deaths = profile.deaths;
        self.kills = profile.kills;
        self.team = profile.team;
        self.scores = profile.scores;
        self.inv.cursor = profile.cursor;
        if let Ok(slots) = profile.slots.try_into() {
            self.inv.slots = slots;
        }
        if let Ok(table) = profile.table.try_into() {
            self.table = table;
        }
    }
    fn snapshot(&self) -> crate::profiles::Profile {
        crate::profiles::Profile {
            slots: self.inv.slots.to_vec(),
            table: self.table.to_vec(),
            cursor: self.inv.cursor,
            xp: self.xp,
            age: self.age,
            deaths: self.deaths,
            kills: self.kills,
            team: self.team.clone(),
            scores: self.scores.clone(),
        }
    }
    pub fn send(&self, p: Packet) {
        if self.output.try_send(p).is_err() {
            self.failed.store(true, Release);
        }
    }
    pub fn tell(&self, s: impl Into<String>) {
        self.send(p::text(s));
    }
    pub fn teleport(&mut self, pos: [f64; 3]) {
        self.pos = pos;
        self.previous = pos;
        self.pending = Some((pos, self.age + 100));
        self.fall = 0.;
        self.moved_tick = 0.;
        self.center = None;
        self.revision += 1;
        self.send(p::position(pos, self.look));
    }
    pub fn harm(&mut self, amount: f32) {
        if amount <= 0. {
            return;
        }
        if self.hp <= 0.
            || matches!(self.mode, 1 | 3)
            || (self.last_hit != 0 && self.age.saturating_sub(self.last_hit) < 10)
        {
            return;
        }
        self.last_hit = self.age.max(1);
        self.hp = (self.hp - amount).max(0.);
        self.send(p::health(self.hp, 20, 5.));
        if self.hp == 0. {
            self.deaths = self.deaths.saturating_add(1);
        }
    }
    fn limited(&mut self, cost: u32) -> bool {
        if self.credit < cost {
            return true;
        }
        self.credit -= cost;
        false
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if let Some(lease) = self.lease.take() {
            lease.finish(self.snapshot());
        }
    }
}

pub struct Lane {
    notices: Sender<Packet>,
    enter: Sender<(Box<Player>, bool)>,
    occupants: AtomicUsize,
}
pub struct Manager {
    pub node_id: uuid::Uuid,
    pub mesh: std::sync::OnceLock<tokio::sync::mpsc::Sender<String>>,
    pub cfg: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub seed: i64,
    lanes: RwLock<BTreeMap<String, Arc<Lane>>>,
    pub template: Arc<Template>,
    pub next: AtomicI32,
    pub stopping: AtomicBool,
    threads: std::sync::Mutex<Vec<std::thread::JoinHandle<()>>>,
}
impl Manager {
    pub fn new(cfg: Config, template: Template, seed: i64) -> io::Result<Arc<Self>> {
        template.spawn()?;
        let mgr = Arc::new(Self {
            node_id: uuid::Uuid::new_v4(),
            mesh: Default::default(),
            cfg: Arc::new(cfg),
            metrics: Arc::new(Metrics::default()),
            seed,
            template: Arc::new(template),
            lanes: RwLock::new(BTreeMap::new()),
            next: AtomicI32::new(1),
            stopping: AtomicBool::new(false),
            threads: Default::default(),
        });
        for name in &mgr.cfg.worlds {
            mgr.create(name)?;
        }
        Ok(mgr)
    }
    pub fn shutdown(&self) {
        self.stopping.store(true, Release);
        let handles = std::mem::take(&mut *self.threads.lock().unwrap());
        for handle in handles {
            if handle.join().is_err() {
                eprintln!("World thread failed");
            }
        }
    }
    pub fn announce(&self, msg: Packet) {
        for l in self.lanes.read().unwrap().values() {
            if l.notices.try_send(msg.clone()).is_err() {
                self.metrics.dropped.fetch_add(1, Relaxed);
            }
        }
    }
    pub fn global_chat(&self, text: String) {
        self.announce(p::text(&text));
        if let Some(tx) = self.mesh.get()
            && tx.try_send(text).is_err()
        {
            self.metrics.dropped.fetch_add(1, Relaxed);
        }
    }
    pub fn names(&self) -> Vec<String> {
        self.lanes.read().unwrap().keys().cloned().collect()
    }
    pub fn create(self: &Arc<Self>, name: &str) -> io::Result<()> {
        if !valid_world(name) {
            return Err(invalid("invalid world name"));
        }
        let mut map = self.lanes.write().unwrap();
        if map.len() >= 128 || map.contains_key(name) {
            return Err(invalid("world exists or capacity reached"));
        }
        let (enter, rx) = crossbeam_channel::bounded(256);
        let (notices, notifications) = crossbeam_channel::bounded(256);
        let lane = Arc::new(Lane {
            enter,
            notices,
            occupants: AtomicUsize::new(0),
        });
        let scripts = self
            .cfg
            .scripts
            .get(name)
            .map(|p| crate::scripts::Scripts::start(p.clone()))
            .transpose()?;
        let index = map.len();
        let mgr = self.clone();
        let local = lane.clone();
        let (store, template) = if self.cfg.persistent_worlds.iter().any(|s| s == name) {
            let (s, t) = crate::persistence::Store::open(
                &self.cfg.data_dir.join(name).join("region"),
                self.template.clone(),
            )?;
            (Some(s), t)
        } else {
            (None, self.template.clone())
        };
        let world = World::new(name.into(), template);
        let handle = std::thread::Builder::new()
            .name(format!("world-{name}"))
            .spawn(move || {
                if let Some(cores) = core_affinity::get_core_ids()
                    && !cores.is_empty()
                {
                    core_affinity::set_for_current(cores[index % cores.len()]);
                }
                Room {
                    world,
                    notifications,
                    scripts,
                    store,
                    circuit: Default::default(),
                    changed: Vec::new(),
                    furnaces: HashMap::new(),
                    autosave: true,
                    mgr,
                    local,
                    players: Vec::new(),
                    last_bulk: None,
                    entities: EntityArena::with_capacity(ENTITIES_PER_ROOM),
                    tick: 0,
                    signs: HashMap::new(),
                    time: 0,
                    weather: false,
                    grid: HashMap::new(),
                }
                .run(rx);
            })?;
        self.threads.lock().unwrap().push(handle);
        map.insert(name.into(), lane);
        Ok(())
    }
    pub fn unload(&self, name: &str) -> io::Result<()> {
        let mut map = self.lanes.write().unwrap();
        let lane = map.get(name).ok_or(invalid("unknown world"))?;
        if name == self.cfg.worlds[0] || lane.occupants.load(Acquire) != 0 {
            return Err(invalid("world is occupied or is the lobby"));
        }
        map.remove(name);
        Ok(())
    }
    pub fn enter(&self, name: &str, p: Box<Player>, transfer: bool) -> Result<(), Box<Player>> {
        let map = self.lanes.read().unwrap();
        let Some(lane) = map.get(name) else {
            return Err(p);
        };
        lane.occupants.fetch_add(1, Relaxed);
        lane.enter.try_send((p, transfer)).map_err(|e| {
            lane.occupants.fetch_sub(1, Release);
            e.into_inner().0
        })
    }
}

pub struct Room {
    notifications: Receiver<Packet>,
    pub world: World,
    pub mgr: Arc<Manager>,
    local: Arc<Lane>,
    pub players: Vec<Player>,
    entities: EntityArena,
    last_bulk: Option<([ColumnPos; 2], Packet)>,
    tick: u64,
    circuit: crate::redstone::Circuit,
    changed: Vec<(BlockPos, u16)>,
    furnaces: HashMap<BlockPos, crate::crafting::Furnace>,
    scripts: Option<crate::scripts::Scripts>,
    pub store: Option<crate::persistence::Store>,
    pub autosave: bool,
    signs: HashMap<BlockPos, [serde_json::Value; 4]>,
    pub time: i64,
    pub weather: bool,
    grid: HashMap<ColumnPos, Vec<usize>>,
}
pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}
impl Room {
    fn run(&mut self, rx: Receiver<(Box<Player>, bool)>) {
        let mut deadline = Instant::now();
        let tick = Duration::from_millis(50);
        while !self.mgr.stopping.load(Acquire) {
            let start = Instant::now();
            // Unloaded lanes have only this thread's Arc left. An entrant owns a reservation.
            if Arc::strong_count(&self.local) == 1 && self.local.occupants.load(Acquire) == 0 {
                break;
            }
            for (p, transfer) in rx.try_iter().take(16) {
                self.join(p, transfer);
            }
            self.tick += 1;
            self.time = self.time.wrapping_add(1);
            let mut i = 0;
            while i < self.players.len() {
                self.players[i].moved_tick = 0.;
                self.players[i].age += 1;
                self.players[i].credit = (self.players[i].credit + 1).min(200);
                let mut transfer = None;
                let mut quit =
                    self.players[i].output.is_closed() || self.players[i].failed.load(Acquire);
                for _ in 0..48 {
                    if quit {
                        break;
                    }
                    match self.players[i].input.try_recv() {
                        Ok(a) => {
                            transfer = self.action(i, a);
                            if transfer.is_some() {
                                break;
                            }
                        }
                        Err(TryRecvError::Disconnected) => {
                            quit = true;
                            break;
                        }
                        Err(TryRecvError::Empty) => break,
                    }
                }
                if let Some(target) = transfer {
                    let mut player = Box::new(self.players.swap_remove(i));
                    let (uuid, id) = (player.identity.uuid, player.id);
                    if target != self.world.name {
                        match self.mgr.enter(&target, player, true) {
                            Ok(()) => {
                                self.depart(uuid, id);
                                continue;
                            }
                            Err(p) => player = p,
                        }
                    }
                    player.tell("World unavailable.");
                    self.players.insert(i, *player);
                    i += 1;
                    continue;
                }
                if quit {
                    let p = self.players.swap_remove(i);
                    self.depart(p.identity.uuid, p.id);
                    continue;
                }
                self.advance(i);
                i += 1;
            }
            self.entities.tick();
            drops::collect_drops(&mut self.entities, &mut self.players);
            self.circuit
                .step(&mut self.world, self.tick, &mut self.changed);
            for &(pos, state) in &self.changed {
                let pkt = p::block(pos, state);
                for p in &self.players {
                    if p.loaded.contains(&(pos.0 >> 4, pos.2 >> 4)) {
                        p.send(pkt.clone());
                    }
                }
            }
            for (&pos, f) in &mut self.furnaces {
                let changed = f.tick();
                for p in &self.players {
                    if p.window == Some(Window::Furnace(pos)) {
                        if changed {
                            for (i, s) in f.slots.iter().enumerate() {
                                p.send(p::slot(1, i as i16, *s));
                            }
                        }
                        if self.tick.is_multiple_of(4) {
                            for (id, value) in
                                [(0, f.burn), (1, f.fuel_time), (2, f.cook), (3, 200)]
                            {
                                p.send(Packet::new(0x31, |w| {
                                    w.put_u8(1);
                                    w.put_i16(id);
                                    w.put_u16(value);
                                }));
                            }
                        }
                    }
                }
            }
            for p in self.notifications.try_iter().take(64) {
                self.broadcast(p);
            }
            self.script_replies();
            self.visibility();
            if self.autosave
                && self.tick.is_multiple_of(1200)
                && let Some(store) = &self.store
            {
                store.save(self.world.edits.clone());
            }
            self.mgr.metrics.tick(start.elapsed().as_micros() as u64);
            deadline += tick;
            let now = Instant::now();
            if deadline < now {
                deadline = now;
            } else {
                std::thread::sleep(deadline - now);
            }
        }
        if let Some(store) = &mut self.store
            && let Err(e) = store.close(self.world.edits.clone())
        {
            eprintln!("Final world save failed: {e}");
        }
    }
    fn join(&mut self, mut player: Box<Player>, transfer: bool) {
        let spawn = self.world.template.spawn().expect("template validated");
        player.window = None;
        player.dig = None;
        if transfer {
            player.send(p::respawn(-1, player.mode));
            player.send(p::respawn(0, player.mode));
        } else {
            player.send(p::join(player.id, player.mode));
        }
        player.loaded.clear();
        player.pending_cols.clear();
        player.seen.clear();
        player.seen_entities.clear();
        player.center = None;
        player.spawn = spawn;
        player.send(p::spawn_point(spawn));
        if player.xp > 0 {
            player.send_xp();
        }
        player.send(p::abilities(player.mode, player.flying));
        player.send(p::health(player.hp, 20, 5.));
        player.send(p::game_state(if self.weather { 1 } else { 2 }, 0.));
        player.send(p::list_add(
            player.identity.uuid,
            &player.identity.name,
            player.mode,
        ));
        for peer in &self.players {
            player.send(p::list_add(
                peer.identity.uuid,
                &peer.identity.name,
                peer.mode,
            ));
        }
        self.broadcast(p::list_add(
            player.identity.uuid,
            &player.identity.name,
            player.mode,
        ));
        for (i, item) in player.inv.slots.iter().enumerate() {
            if item.is_some() {
                player.send(p::slot(0, i as i16, *item));
            }
        }
        player.send(p::slot(-1, -1, player.inv.cursor));
        player.teleport(spawn);
        self.players.push(*player);
        self.notify_script(self.players.len() - 1, "join", 0);
    }
    fn notify_script(&self, i: usize, kind: &'static str, block: u16) {
        if let Some(s) = &self.scripts {
            let p = &self.players[i];
            let _ = s.events.try_send(crate::scripts::Event {
                kind,
                player: p.id,
                name: p.identity.name.clone(),
                pos: p.pos,
                block,
            });
        }
    }
    fn script_replies(&mut self) {
        for _ in 0..32 {
            let Some(reply) = self
                .scripts
                .as_ref()
                .and_then(|s| s.replies.try_recv().ok())
            else {
                break;
            };
            let Some(p) = self.players.iter_mut().find(|p| p.id == reply.player) else {
                continue;
            };
            if let Some(s) = reply.message {
                p.tell(s);
            }
            if let Some(mode) = reply.mode {
                p.mode = mode;
                p.flying = mode == 3;
                p.send(p::game_state(3, mode as f32));
                p.send(p::abilities(mode, p.flying));
            }
            if let Some(stack) = reply.kit {
                let before = p.inv.slots;
                if p.inv.add(stack) {
                    for (i, item) in p.inv.slots.iter().enumerate() {
                        if before[i] != *item {
                            p.send(p::slot(0, i as i16, *item));
                        }
                    }
                }
            }
            if let Some(s) = reply.broadcast {
                self.broadcast(p::text(s));
            }
        }
    }
    fn depart(&mut self, uuid: uuid::Uuid, id: i32) {
        let remove = p::list_remove(uuid);
        let destroy = p::destroy(id);
        for other in &mut self.players {
            other.send(remove.clone());
            if other.seen.remove(&id).is_some() {
                other.send(destroy.clone());
            }
        }
        self.local.occupants.fetch_sub(1, Release);
    }
    pub fn broadcast(&self, p: Packet) {
        for player in &self.players {
            player.send(p.clone());
        }
    }
    pub fn set_block(&mut self, pos: BlockPos, state: u16) -> io::Result<()> {
        self.world.set(pos, state)?;
        self.circuit.changed(pos);
        self.signs.remove(&pos);
        self.furnaces.remove(&pos);
        let col = (pos.0 >> 4, pos.2 >> 4);
        let packet = p::block(pos, state);
        for peer in &self.players {
            if peer.loaded.contains(&col) {
                peer.send(packet.clone());
            }
        }
        Ok(())
    }
    pub fn edit_many(&mut self, edits: Vec<(BlockPos, u16)>) -> io::Result<()> {
        let mut by_col: BTreeMap<ColumnPos, Vec<(BlockPos, u16)>> = BTreeMap::new();
        for (pos, state) in edits {
            self.world.set(pos, state)?;
            self.circuit.changed(pos);
            self.signs.remove(&pos);
            self.furnaces.remove(&pos);
            by_col
                .entry((pos.0 >> 4, pos.2 >> 4))
                .or_default()
                .push((pos, state));
        }
        for ((cx, cz), records) in by_col {
            let packet = Packet::new(0x22, |w| {
                w.put_i32(cx);
                w.put_i32(cz);
                w.var(records.len() as i32);
                for ((x, y, z), state) in records {
                    w.put_u8((((x & 15) << 4) | (z & 15)) as u8);
                    w.put_u8(y);
                    w.var(i32::from(state));
                }
            });
            for a in &self.players {
                if a.loaded.contains(&(cx, cz)) {
                    a.send(packet.clone());
                }
            }
        }
        Ok(())
    }
    fn advance(&mut self, i: usize) {
        let player = &mut self.players[i];
        if player.pending.is_some_and(|(_, t)| player.age >= t)
            || player.keep.is_some_and(|(_, t)| player.age - t > 600)
        {
            player.failed.store(true, Release);
            return;
        }
        if player.age.is_multiple_of(200) && player.keep.is_none() {
            let token = player.age as i32;
            player.keep = Some((token, player.age));
            player.send(Packet::new(0, |w| w.var(token)));
        }
        if player.age.is_multiple_of(200) {
            let snapshot = player.snapshot();
            if let Some(l) = &mut player.lease {
                l.checkpoint(snapshot);
            }
        }
        if player.age.is_multiple_of(20) {
            player.send(Packet::new(3, |w| {
                w.put_i64(self.tick as i64);
                w.put_i64(self.time);
            }));
        }
        if player.pos[1] < -64. && player.age.is_multiple_of(10) {
            player.harm(4.);
        }
        if player.hp > 0. && player.hp < 20. && player.age.is_multiple_of(80) {
            player.hp = (player.hp + 1.).min(20.);
            player.send(p::health(player.hp, 20, 5.));
        }
        let c = column(player.pos);
        let radius = i32::from(self.mgr.cfg.view_distance);
        if player.center != Some(c) {
            player.center = Some(c);
            player.pending_cols.clear();
            let out = &player.output;
            let failed = &player.failed;
            player.loaded.retain(|&(x, z)| {
                let keep = (x - c.0).abs() <= radius && (z - c.1).abs() <= radius;
                if !keep && out.try_send(p::unload((x, z))).is_err() {
                    failed.store(true, Release);
                }
                keep
            });
            for x in c.0 - radius..=c.0 + radius {
                for z in c.1 - radius..=c.1 + radius {
                    if !player.loaded.contains(&(x, z)) && self.world.template.contains((x, z)) {
                        player.pending_cols.push((x, z));
                    }
                }
            }
            let yaw = f64::from(player.look[0]).to_radians();
            let dir = [
                -yaw.sin() + player.pos[0] - player.previous[0],
                yaw.cos() + player.pos[2] - player.previous[2],
            ];
            player.pending_cols.sort_unstable_by_key(|&(x, z)| {
                let dx = f64::from(x - c.0);
                let dz = f64::from(z - c.1);
                -((dx * dx + dz * dz - (dx * dir[0] + dz * dir[1]).clamp(-0.9, 0.9)) * 100.) as i32
            });
        }
        let cols = [player.pending_cols.pop(), player.pending_cols.pop()];
        for col in cols.into_iter().flatten() {
            self.mgr.metrics.chunk_lookups.fetch_add(1, Relaxed);
            if self.world.template.cached(col) {
                self.mgr.metrics.chunk_hits.fetch_add(1, Relaxed);
            }
        }
        let mut grouped = false;
        if let [Some(a), Some(b)] = cols
            && a.0.abs_diff(b.0) + a.1.abs_diff(b.1) == 1
        {
            let positions = [a, b];
            if self
                .last_bulk
                .as_ref()
                .is_none_or(|(key, _)| *key != positions)
            {
                self.last_bulk = self
                    .world
                    .template
                    .bulk_packet(positions)
                    .map(|p| (positions, p));
            }
            if let Some((_, packet)) = &self.last_bulk {
                player.send(packet.clone());
                grouped = true;
            }
        }
        for col in cols.into_iter().flatten() {
            if !grouped {
                let Some(packet) = self.world.template.packet(col) else {
                    continue;
                };
                player.send(packet);
            }
            player.loaded.insert(col);
            if let Some(edits) = self.world.edits.get(&col) {
                for (&pos, &state) in edits {
                    player.send(p::block(pos, state));
                    if let Some(lines) = self.signs.get(&pos) {
                        player.send(p::sign(pos, lines));
                    }
                }
            }
        }
    }
    fn action(&mut self, i: usize, a: Action) -> Option<String> {
        match a {
            Action::Chat(s) => {
                if !self.players[i].limited(20) {
                    if let Some(cmd) = s.strip_prefix('/') {
                        return crate::commands::execute(self, i, cmd);
                    }
                    let msg = format!("<{}> {}", self.players[i].identity.name, s);
                    println!("{msg}");
                    self.mgr.global_chat(msg);
                }
            }
            Action::Complete(s) => {
                if !self.players[i].limited(4) {
                    self.players[i].send(p::completions(&crate::commands::complete(self, i, &s)));
                }
            }
            Action::KeepAlive(id) => {
                if self.players[i].keep.is_some_and(|(v, _)| v == id) {
                    self.players[i].keep = None;
                }
            }
            Action::Stats => {
                let a = &self.players[i];
                a.send(p::stats(&[
                    ("stat.playOneMinute", a.age.min(i32::MAX as u64) as i32),
                    ("stat.deaths", a.deaths),
                    ("stat.playerKills", a.kills),
                ]));
            }
            Action::Respawn => {
                let a = &mut self.players[i];
                if a.hp <= 0. {
                    a.hp = 20.;
                    a.send(p::respawn(0, a.mode));
                    a.send(p::health(20., 20, 5.));
                    a.loaded.clear();
                    a.seen.clear();
                    a.seen_entities.clear();
                    a.teleport(a.spawn);
                }
            }
            Action::Move { pos, look, ground } => self.movement(i, pos, look, ground),
            Action::Sprint(b) => self.players[i].sprint = b,
            Action::Ability(b) => {
                let a = &mut self.players[i];
                a.flying = a.mode == 3 || (a.mode == 1 && b);
            }
            Action::Held(s) => self.players[i].inv.held = s,
            Action::Creative(idx, item) => {
                let a = &mut self.players[i];
                if a.mode == 1 && (9..45).contains(&idx) {
                    a.inv.slots[idx as usize] = item;
                    a.send(p::slot(0, idx, item));
                }
            }
            Action::Click {
                window,
                slot,
                button,
                sequence,
                mode,
            } => {
                self.window_click(i, window, slot, button, sequence, mode);
            }
            Action::Close(window) => {
                if window == 1 {
                    self.players[i].window = None;
                }
            }
            Action::Dig(status, pos) => {
                let a = &mut self.players[i];
                if a.hp <= 0. || !matches!(a.mode, 0 | 1) || !reachable(a, pos) {
                    return None;
                }
                let before = self.world.block(pos);
                let id = before >> 4;
                if before == 0 || (id == 7 && a.mode != 1) {
                    return None;
                }
                let duration = match id {
                    2 | 3 | 12 | 13 => 12,
                    5 | 17 => 40,
                    1 | 4 => 100,
                    _ => 24,
                };
                let complete = match (a.mode, status) {
                    (1, 0) => true,
                    (0, 0) => {
                        a.dig = Some((pos, self.tick, before));
                        false
                    }
                    (0, 2) => a.dig.take().is_some_and(|(p, t, b)| {
                        p == pos && b == before && self.tick - t >= duration
                    }),
                    _ => {
                        a.dig = None;
                        false
                    }
                };
                if complete {
                    let survival = a.mode == 0;
                    self.break_block(i, pos, before, survival);
                }
            }
            Action::Place(pos, face) => {
                if !reachable(&self.players[i], pos) || self.players[i].hp <= 0. {
                    return None;
                }
                let block = self.world.block(pos);
                let id = block >> 4;
                if matches!(id, 58 | 61 | 62) {
                    self.open_window(i, pos, id);
                    return None;
                }
                if matches!(id, 69 | 77) {
                    let _ = self.set_block(pos, block ^ 8);
                    return None;
                }
                let a = &self.players[i];
                if matches!(a.mode, 0 | 1) {
                    let offset = [
                        (0, -1, 0),
                        (0, 1, 0),
                        (0, 0, -1),
                        (0, 0, 1),
                        (-1, 0, 0),
                        (1, 0, 0),
                    ][face as usize];
                    let slot = 36 + usize::from(a.inv.held);
                    if let Ok(y) = u8::try_from(i32::from(pos.1) + offset.1)
                        && let Some(Stack { id, damage, .. }) = a.inv.slots[slot]
                        && (1..=197).contains(&id)
                    {
                        let at = (pos.0 + offset.0, y, pos.2 + offset.2);
                        let state = ((id as u16) << 4) | (damage as u16 & 15);
                        if self.world.block(at) == 0 && self.set_block(at, state).is_ok() {
                            let a = &mut self.players[i];
                            if a.mode == 0 {
                                crate::crafting::consume(&mut a.inv.slots[slot]);
                                a.send(p::slot(0, slot as i16, a.inv.slots[slot]));
                            }
                            self.notify_script(i, "place", state);
                        }
                    }
                }
            }
            Action::Attack(id) => self.attack(i, id),
            Action::Swing => {
                let id = self.players[i].id;
                let packet = Packet::new(0x0b, |w| {
                    w.var(id);
                    w.put_u8(0);
                });
                for a in &self.players {
                    if a.seen.contains_key(&id) {
                        a.send(packet.clone());
                    }
                }
            }
            Action::Sign(pos, lines) => {
                let a = &self.players[i];
                if a.mode == 1 && reachable(a, pos) && matches!(self.world.block(pos) >> 4, 63 | 68)
                {
                    let lines = lines.map(
                        |s| serde_json::json!({"text":s.chars().take(16).collect::<String>()}),
                    );
                    self.broadcast(p::sign(pos, &lines));
                    self.signs.insert(pos, lines);
                }
            }
            Action::Ignore => {}
        }
        None
    }
    fn movement(
        &mut self,
        i: usize,
        pos: Option<[f64; 3]>,
        look: Option<[f32; 2]>,
        _reported_ground: bool,
    ) {
        let a = &mut self.players[i];
        if a.hp <= 0. {
            return;
        }
        if let Some((target, _)) = a.pending {
            if !pos.is_some_and(|p| distance(p, target) < 0.0625) {
                return;
            }
            a.pending = None;
        }
        if let Some(look) = look {
            if look != a.look {
                a.revision += 1;
            }
            a.look = look;
        }
        let Some(pos) = pos else {
            return;
        };
        let moved = distance(a.pos, pos).sqrt();
        if a.moved_tick + moved > 10. || (a.mode != 3 && body_blocked(&self.world, pos)) {
            a.teleport(a.pos);
            return;
        }
        a.moved_tick += moved;
        let ground = solid(&self.world, [pos[0], pos[1] - 0.05, pos[2]]);
        if !a.flying {
            a.fall += (a.pos[1] - pos[1]).max(0.);
            if ground && !a.ground {
                a.harm((a.fall - 3.).ceil().max(0.) as f32);
            }
            if ground || pos[1] > a.pos[1] {
                a.fall = 0.;
            }
        }
        a.ground = ground;
        a.previous = a.pos;
        if moved > 0. {
            a.revision += 1;
        }
        a.pos = pos;
    }
    fn attack(&mut self, i: usize, id: i32) {
        let Some(j) = self.players.iter().position(|a| a.id == id) else {
            return;
        };
        let a = &self.players[i];
        let b = &self.players[j];
        if i == j
            || a.hp <= 0.
            || a.mode == 3
            || b.hp <= 0.
            || matches!(b.mode, 1 | 3)
            || distance(a.pos, b.pos) > 16.
            || !line_clear(&self.world, a.pos, b.pos)
        {
            return;
        }
        let dx = b.pos[0] - a.pos[0];
        let dz = b.pos[2] - a.pos[2];
        let d = dx.hypot(dz).max(0.001);
        let k = if a.sprint { 0.8 } else { 0.4 };
        let crit = a.fall > 0. && !a.ground && !a.flying;
        let dmg = match a.inv.slots[36 + usize::from(a.inv.held)].map(|s| s.id) {
            Some(276) => 8.,
            Some(267) => 7.,
            Some(272) => 6.,
            Some(268 | 283) => 5.,
            _ => 1.,
        };
        let before = b.hp;
        let b = &mut self.players[j];
        b.harm(if crit { dmg * 1.5 } else { dmg });
        if b.hp < before {
            b.send(p::velocity(id, [dx / d * k, 0.4, dz / d * k]));
            let dead = b.hp == 0.;
            self.players[i].sprint = false;
            if dead {
                drops::drop_xp(&mut self.entities, &mut self.players[j]);
                self.players[i].kills += 1;
                self.notify_script(i, "kill", 0);
                self.notify_script(j, "death", 0);
            }
            self.broadcast(Packet::new(0x1a, |w| {
                w.put_i32(id);
                w.put_u8(if dead { 3 } else { 2 });
            }));
        }
    }
    fn visibility(&mut self) {
        for v in self.grid.values_mut() {
            v.clear();
        }
        for (i, p) in self.players.iter().enumerate() {
            self.grid.entry(column(p.pos)).or_default().push(i);
        }
        entity_visibility::broadcast(&self.entities, &self.grid, &mut self.players);
        for i in 0..self.players.len() {
            let viewer = &self.players[i];
            let c = column(viewer.pos);
            let origin = viewer.pos;
            let mut updates = Vec::new();
            let mut observed = HashSet::new();
            for col in &viewer.loaded {
                if (col.0 - c.0).abs() > 5 || (col.1 - c.1).abs() > 5 {
                    continue;
                }
                for &j in self.grid.get(col).into_iter().flatten() {
                    if j == i {
                        continue;
                    }
                    let target = &self.players[j];
                    let d = distance(origin, target.pos);
                    if d > 80. * 80. {
                        continue;
                    }
                    let interval = if d < 256. {
                        1
                    } else if d < 48. * 48. {
                        2
                    } else {
                        4
                    };
                    let known = viewer.seen.get(&target.id);
                    // Retain the last absolute position between visibility evaluations.
                    if known.is_some() && !self.tick.is_multiple_of(interval) {
                        observed.insert(target.id);
                        continue;
                    }
                    if d >= 256. && !line_clear(&self.world, origin, target.pos) {
                        continue;
                    }
                    observed.insert(target.id);
                    if known.is_none() {
                        viewer.send(p::spawn_player(
                            target.id,
                            target.identity.uuid,
                            target.pos,
                            target.look,
                        ));
                    } else if known != Some(&target.revision) {
                        viewer.send(p::entity_pos(
                            target.id,
                            target.pos,
                            target.look,
                            target.ground,
                        ));
                        viewer.send(p::head(target.id, target.look[0]));
                    }
                    updates.push((target.id, target.revision));
                }
            }
            for &id in viewer.seen.keys() {
                if !observed.contains(&id) {
                    viewer.send(p::destroy(id));
                }
            }
            let seen = &mut self.players[i].seen;
            seen.retain(|id, _| observed.contains(id));
            seen.extend(updates);
        }
    }
}
fn reachable(a: &Player, (x, y, z): BlockPos) -> bool {
    distance(
        [a.pos[0], a.pos[1] + 1.62, a.pos[2]],
        [x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5],
    ) <= 36.
}
fn solid(world: &World, p: [f64; 3]) -> bool {
    (0.0..256.).contains(&p[1])
        && matches!(world.block((p[0].floor() as i32,p[1].floor() as u8,p[2].floor() as i32))>>4,1..=5|7|12..=17|21..=25|35|41..=43|45..=49|56..=58|73..=74|80|82|87..=89|98|112|121|133|152|155|159|172..=174)
}
fn body_blocked(w: &World, p: [f64; 3]) -> bool {
    solid(w, p) || solid(w, [p[0], p[1] + 1.62, p[2]])
}
fn line_clear(w: &World, a: [f64; 3], b: [f64; 3]) -> bool {
    let steps = (distance(a, b).sqrt() * 2.).ceil() as usize;
    (1..steps).all(|i| {
        let t = i as f64 / steps as f64;
        !solid(
            w,
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + 1.5 + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ],
        )
    })
}

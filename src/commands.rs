use crate::{
    arena::BlockPos,
    engine::{Player, Room},
    packets::{self as p, Stack},
    selector::Selector,
    wire::{WriteWire, invalid},
};
use bytes::BufMut;
use serde_json::Value;
use std::io;

struct Command(&'static str, u8, &'static str);
const REGISTRY: &[Command] = &[
    Command("save-all", 4, ""),
    Command("save-on", 4, ""),
    Command("save-off", 4, ""),
    Command("help", 0, "[command]"),
    Command("list", 0, ""),
    Command("me", 0, "<action>"),
    Command(
        "world",
        0,
        "list | tp <world> | clone <name> | unload <world>",
    ),
    Command("seed", 2, ""),
    Command("say", 1, "<message>"),
    Command("tellraw", 2, "<target> <json>"),
    Command("title", 2, "<target> title|subtitle|clear|reset|times ..."),
    Command("gamemode", 2, "<mode> [target]"),
    Command("tp", 2, "<target> <x> <y> <z>"),
    Command("give", 2, "<target> <item-id> [count] [damage]"),
    Command("clear", 2, "[target]"),
    Command("xp", 2, "<points> [target]"),
    Command("kill", 2, "[target]"),
    Command("spawnpoint", 2, "[target] [x y z]"),
    Command("setblock", 2, "<x> <y> <z> <block-id> [meta]"),
    Command("fill", 2, "<x1> <y1> <z1> <x2> <y2> <z2> <block-id> [meta]"),
    Command("clone", 2, "<x1> <y1> <z1> <x2> <y2> <z2> <x> <y> <z>"),
    Command("testfor", 2, "<target>"),
    Command("testforblock", 2, "<x> <y> <z> <block-id> [meta]"),
    Command("weather", 2, "clear|rain"),
    Command("time", 2, "set|add <ticks>"),
    Command("particle", 2, "<id> <x> <y> <z> <count> [data ...]"),
    Command("playsound", 2, "<sound> <target> [x y z]"),
    Command(
        "scoreboard",
        2,
        "players set|add <target> <objective> <value>",
    ),
    Command("team", 2, "join <team> <target> | leave <target>"),
];
fn canonical(s: &str) -> &str {
    match s {
        "?" => "help",
        "teleport" => "tp",
        "experience" => "xp",
        "gm" => "gamemode",
        _ => s,
    }
}
fn allowed(room: &Room, sender: &Player, name: &str, lvl: u8) -> bool {
    let uuid = sender.identity.uuid;
    room.mgr.cfg.operators.get(&uuid).copied().unwrap_or(0) >= lvl
        || room.mgr.cfg.permissions.get(&uuid).is_some_and(|nodes| {
            nodes
                .iter()
                .any(|n| n.strip_prefix("coral.command.") == Some(name))
        })
}
fn targets(room: &Room, i: usize, s: Option<&str>) -> io::Result<Vec<usize>> {
    let Some(s) = s else {
        return Ok(vec![i]);
    };
    let list = Selector::parse(s)?.resolve(room, &room.players[i]);
    if list.is_empty() {
        Err(invalid("No matching player"))
    } else {
        Ok(list)
    }
}
fn integer(s: Option<&&str>) -> io::Result<i32> {
    s.ok_or(invalid("Missing integer"))?
        .parse()
        .map_err(|_| invalid("Invalid integer"))
}
fn coord(s: &str, base: f64) -> io::Result<f64> {
    let n = if let Some(rel) = s.strip_prefix('~') {
        base + if rel.is_empty() {
            0.
        } else {
            rel.parse()
                .map_err(|_| invalid("Invalid relative coordinate"))?
        }
    } else {
        s.parse().map_err(|_| invalid("Invalid coordinate"))?
    };
    if n.abs() <= 30_000_000. {
        Ok(n)
    } else {
        Err(invalid("Coordinate outside world"))
    }
}
fn position(args: &[&str], base: [f64; 3]) -> io::Result<[f64; 3]> {
    if args.len() < 3 {
        return Err(invalid("Expected three coordinates"));
    }
    Ok([
        coord(args[0], base[0])?,
        coord(args[1], base[1])?,
        coord(args[2], base[2])?,
    ])
}
fn block_pos(args: &[&str], base: [f64; 3]) -> io::Result<BlockPos> {
    let [x, y, z] = position(args, base)?;
    if !(0.0..256.).contains(&y) {
        return Err(invalid("Invalid height"));
    }
    Ok((x.floor() as i32, y as u8, z.floor() as i32))
}
fn state(args: &[&str]) -> io::Result<u16> {
    let id = integer(args.first())?;
    let meta = if args.len() > 1 {
        integer(args.get(1))?
    } else {
        0
    };
    if !(0..=197).contains(&id) || !(0..=15).contains(&meta) {
        return Err(invalid("Invalid block state"));
    }
    Ok((id * 16 + meta) as u16)
}
impl Player {
    pub fn level(&self) -> i32 {
        let xp = self.xp as f64;
        if xp < 352. {
            ((xp + 9.).sqrt() - 3.).floor() as i32
        } else if xp < 1507. {
            ((40.5 + (40.5_f64.powi(2) - 10. * (360. - xp)).sqrt()) / 5.).floor() as i32
        } else {
            ((162.5 + (162.5_f64.powi(2) - 18. * (2220. - xp)).sqrt()) / 9.).floor() as i32
        }
    }
}
fn xp_for_level(l: i32) -> f64 {
    let l = l as f64;
    if l <= 16. {
        l * l + 6. * l
    } else if l <= 31. {
        2.5 * l * l - 40.5 * l + 360.
    } else {
        4.5 * l * l - 162.5 * l + 2220.
    }
}

pub fn execute(room: &mut Room, i: usize, line: &str) -> Option<String> {
    match run(room, i, line) {
        Ok(target) => target,
        Err(e) => {
            room.players[i].tell(e.to_string());
            None
        }
    }
}
fn run(room: &mut Room, i: usize, line: &str) -> io::Result<Option<String>> {
    let words: Vec<_> = line.split_whitespace().collect();
    let Some(first) = words.first() else {
        return Ok(None);
    };
    let lower = first.to_ascii_lowercase();
    let name = canonical(&lower);
    let command = REGISTRY
        .iter()
        .find(|c| c.0 == name)
        .ok_or(invalid("Unknown command"))?;
    if !allowed(room, &room.players[i], name, command.1) {
        return Err(invalid("Permission denied"));
    }
    let a = &words[1..];
    let arg = |n: usize| a.get(n).copied().ok_or(invalid("Missing command argument"));
    let base = room.players[i].pos;
    match name {
        "help" => {
            if let Some(n) = a.first() {
                let c = REGISTRY
                    .iter()
                    .find(|c| c.0 == canonical(n) && allowed(room, &room.players[i], c.0, c.1))
                    .ok_or(invalid("Unknown command"))?;
                room.players[i].tell(format!("/{} {}", c.0, c.2));
            } else {
                room.players[i].tell(
                    REGISTRY
                        .iter()
                        .filter(|c| allowed(room, &room.players[i], c.0, c.1))
                        .map(|c| c.0)
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }
        }
        "list" => room.players[i].tell(
            room.players
                .iter()
                .map(|p| p.identity.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        "me" | "say" => {
            arg(0)?;
            room.broadcast(p::text(format!(
                "{}{} {}",
                if name == "me" { "* " } else { "" },
                room.players[i].identity.name,
                a.join(" ")
            )));
        }
        "save-all" | "save-on" | "save-off" => {
            let store = room
                .store
                .as_ref()
                .ok_or(invalid("This world is ephemeral"))?;
            if name == "save-all" {
                let ok = store.save(room.world.edits.clone());
                room.players[i].tell(if ok { "Save queued" } else { "Save queue busy" });
            } else {
                room.autosave = name == "save-on";
                room.players[i].tell("Autosave setting updated");
            }
        }
        "seed" => room.players[i].tell(room.mgr.seed.to_string()),
        "world" => match a.first().copied().unwrap_or("list") {
            "list" => room.players[i].tell(room.mgr.names().join(", ")),
            "tp" => return Ok(Some(arg(1)?.into())),
            action @ ("clone" | "load" | "unload") => {
                if !allowed(room, &room.players[i], "world.manage", 3) {
                    return Err(invalid("Permission denied"));
                }
                if action == "unload" {
                    room.mgr.unload(arg(1)?)?;
                } else {
                    room.mgr.create(arg(1)?)?;
                }
                room.players[i].tell("World list updated.");
            }
            _ => return Err(invalid("Unknown world operation")),
        },
        "testfor" => {
            let n = targets(room, i, Some(arg(0)?))?.len();
            room.players[i].tell(format!("{n} matching player(s)"));
        }
        "tp" | "spawnpoint" => {
            let t = targets(room, i, a.first().copied())?;
            let dest = if a.len() <= 1 && name == "spawnpoint" {
                base
            } else {
                position(a.get(1..).unwrap_or_default(), base)?
            };
            for j in t {
                if name == "tp" {
                    room.players[j].teleport(dest);
                } else {
                    room.players[j].spawn = dest;
                    room.players[j].send(p::spawn_point(dest));
                }
            }
        }
        "kill" | "clear" => {
            for j in targets(room, i, a.first().copied())? {
                let p = &mut room.players[j];
                if name == "kill" {
                    if p.hp > 0. {
                        p.deaths += 1;
                    }
                    p.hp = 0.;
                    p.send(p::health(0., 20, 5.));
                    p.send(crate::wire::Packet::new(0x1a, |w| {
                        w.put_i32(p.id);
                        w.put_u8(3);
                    }));
                } else {
                    p.inv = Default::default();
                    for s in 0..45 {
                        p.send(p::slot(0, s, None));
                    }
                    p.send(p::slot(-1, -1, None));
                }
            }
        }
        "gamemode" => {
            let mode = match arg(0)? {
                "survival" | "0" => 0,
                "creative" | "1" => 1,
                "adventure" | "2" => 2,
                "spectator" | "3" => 3,
                _ => return Err(invalid("Invalid game mode")),
            };
            for j in targets(room, i, a.get(1).copied())? {
                let p = &mut room.players[j];
                p.mode = mode;
                p.flying = mode == 3;
                p.send(p::game_state(3, f32::from(mode)));
                p.send(p::abilities(mode, p.flying));
            }
        }
        "xp" => {
            let n = integer(a.first())?;
            for j in targets(room, i, a.get(1).copied())? {
                let p = &mut room.players[j];
                p.xp = p.xp.saturating_add(n).max(0);
                let level = p.level();
                let progress = ((p.xp as f64 - xp_for_level(level))
                    / (xp_for_level(level + 1) - xp_for_level(level)))
                    as f32;
                p.send(crate::wire::Packet::new(0x1f, |w| {
                    w.put_f32(progress);
                    w.var(level);
                    w.var(p.xp);
                }));
            }
        }
        "give" => {
            let t = targets(room, i, Some(arg(0)?))?;
            let id = integer(a.get(1))?;
            let n = if a.len() > 2 { integer(a.get(2))? } else { 1 };
            let damage = if a.len() > 3 { integer(a.get(3))? } else { 0 };
            if !(1..=431).contains(&id) || !(1..=64).contains(&n) || !(0..=32767).contains(&damage)
            {
                return Err(invalid("Invalid stack"));
            }
            for j in t {
                let p = &mut room.players[j];
                let before = p.inv.slots;
                if !p.inv.add(Stack {
                    id: id as i16,
                    count: n as u8,
                    damage: damage as i16,
                }) {
                    p.tell("Inventory full");
                    continue;
                }
                for (n, item) in p.inv.slots.iter().enumerate() {
                    if before[n] != *item {
                        p.send(p::slot(0, n as i16, *item));
                    }
                }
            }
        }
        "setblock" | "testforblock" => {
            let at = block_pos(a, base)?;
            let value = state(&a[3..])?;
            if name == "setblock" {
                room.set_block(at, value)?;
            } else {
                room.players[i].tell(if room.world.block(at) == value {
                    "Block matches"
                } else {
                    "Block differs"
                });
            }
        }
        "fill" | "clone" => {
            let from = block_pos(a, base)?;
            let to = block_pos(&a[3..], base)?;
            let min = (from.0.min(to.0), from.1.min(to.1), from.2.min(to.2));
            let max = (from.0.max(to.0), from.1.max(to.1), from.2.max(to.2));
            let volume = i64::from(max.0 - min.0 + 1)
                * i64::from(max.1 - min.1 + 1)
                * i64::from(max.2 - min.2 + 1);
            if volume > 4096 {
                return Err(invalid("Volume exceeds the 4096 block edit budget"));
            }
            let dest = if name == "clone" {
                block_pos(&a[6..], base)?
            } else {
                min
            };
            let value = if name == "fill" {
                Some(state(&a[6..])?)
            } else {
                None
            };
            let mut writes = Vec::with_capacity(volume as usize);
            for x in min.0..=max.0 {
                for z in min.2..=max.2 {
                    for y in min.1..=max.1 {
                        let target = (
                            dest.0 + x - min.0,
                            dest.1
                                .checked_add(y - min.1)
                                .ok_or(invalid("Invalid destination height"))?,
                            dest.2 + z - min.2,
                        );
                        if !room.world.template.contains((target.0 >> 4, target.2 >> 4)) {
                            return Err(invalid("Destination outside arena"));
                        }
                        writes.push((target, value.unwrap_or_else(|| room.world.block((x, y, z)))));
                    }
                }
            }
            room.edit_many(writes)?;
        }
        "tellraw" | "title" => {
            let t = targets(room, i, Some(arg(0)?))?;
            let packet = if name == "tellraw" {
                let v: Value = serde_json::from_str(&a[1..].join(" ")).map_err(io::Error::other)?;
                p::chat(&v)
            } else {
                let action = match arg(1)? {
                    "title" => 0,
                    "subtitle" => 1,
                    "times" => 2,
                    "clear" => 3,
                    "reset" => 4,
                    _ => return Err(invalid("Unknown title action")),
                };
                let text = if action < 2 {
                    let v: Value =
                        serde_json::from_str(&a[2..].join(" ")).map_err(io::Error::other)?;
                    v.to_string()
                } else {
                    String::new()
                };
                let times = if action == 2 {
                    [integer(a.get(2))?, integer(a.get(3))?, integer(a.get(4))?]
                } else {
                    [0; 3]
                };
                crate::wire::Packet::new(0x45, |w| {
                    w.var(action);
                    if action < 2 {
                        w.string(&text);
                    }
                    if action == 2 {
                        for n in times {
                            w.put_i32(n);
                        }
                    }
                })
            };
            for j in t {
                room.players[j].send(packet.clone());
            }
        }
        "weather" => {
            room.weather = match arg(0)? {
                "rain" => true,
                "clear" => false,
                _ => return Err(invalid("Expected clear or rain")),
            };
            room.broadcast(p::game_state(if room.weather { 1 } else { 2 }, 0.));
        }
        "time" => {
            let n = i64::from(integer(a.get(1))?);
            room.time = match arg(0)? {
                "set" => n,
                "add" => room.time.saturating_add(n),
                _ => return Err(invalid("Expected set or add")),
            };
        }
        "particle" => {
            let id = integer(a.first())?;
            let pos = position(&a[1..], base)?;
            let count = integer(a.get(4))?;
            let extra = a[5..]
                .iter()
                .map(|v| integer(Some(v)))
                .collect::<io::Result<Vec<_>>>()?;
            room.broadcast(p::particle(id, pos, count, &extra)?);
        }
        "playsound" => {
            let sound = arg(0)?;
            let t = targets(room, i, Some(arg(1)?))?;
            let pos = if a.len() > 2 {
                position(&a[2..], base)?
            } else {
                base
            };
            let packet = crate::wire::Packet::new(0x29, |w| {
                w.string(sound);
                for v in pos {
                    w.put_i32((v * 8.) as i32);
                }
                w.put_f32(1.);
                w.put_u8(63);
            });
            for j in t {
                room.players[j].send(packet.clone());
            }
        }
        "scoreboard" => {
            if arg(0)? != "players" {
                return Err(invalid("Expected players"));
            }
            let op = arg(1)?;
            let t = targets(room, i, Some(arg(2)?))?;
            let objective = arg(3)?;
            if objective.len() > 16 {
                return Err(invalid("Objective too long"));
            }
            let n = integer(a.get(4))?;
            for j in t {
                let v = room.players[j].scores.entry(objective.into()).or_default();
                *v = match op {
                    "set" => n,
                    "add" => v.saturating_add(n),
                    _ => return Err(invalid("Expected set or add")),
                };
            }
        }
        "team" => {
            let (team, target) = match arg(0)? {
                "join" => (arg(1)?, arg(2)?),
                "leave" => ("", arg(1)?),
                _ => return Err(invalid("Expected join or leave")),
            };
            if team.len() > 16 {
                return Err(invalid("Team name too long"));
            }
            for j in targets(room, i, Some(target))? {
                room.players[j].team = team.into();
            }
        }
        _ => return Err(invalid("Unknown command")),
    }
    Ok(None)
}
pub fn complete(room: &Room, i: usize, text: &str) -> Vec<String> {
    let word = text.split(' ').next_back().unwrap_or("");
    let head = text.split_whitespace().next().unwrap_or("");
    let mut candidates = Vec::new();
    if text.starts_with('/') && !text.contains(' ') {
        candidates.extend(
            REGISTRY
                .iter()
                .filter(|c| allowed(room, &room.players[i], c.0, c.1))
                .map(|c| format!("/{}", c.0)),
        );
    } else if head == "/world" && text.starts_with("/world tp ") {
        candidates = room.mgr.names();
    } else if head == "/world" {
        candidates.extend(["list", "tp"].map(String::from));
        if allowed(room, &room.players[i], "world.manage", 3) {
            candidates.extend(["clone", "unload"].map(String::from));
        }
    } else if matches!(head, "/gamemode" | "/gm") && text.matches(' ').count() == 1 {
        candidates.extend(["survival", "creative", "adventure", "spectator"].map(String::from));
    } else {
        candidates.extend(room.players.iter().map(|p| p.identity.name.clone()));
        if head.starts_with('/') {
            candidates.extend(["@a", "@p", "@r", "@e"].map(String::from));
        }
    }
    candidates.retain(|s| {
        s.get(..word.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(word))
    });
    candidates.sort_unstable();
    candidates.truncate(256);
    candidates
}

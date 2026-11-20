use crate::engine::{Player, Room, distance};
use crate::wire::invalid;
use std::io;

#[derive(Clone, Copy)]
pub struct Selector<'a> {
    kind: u8,
    args: &'a str,
    literal: &'a str,
}
fn number(s: &str) -> io::Result<f64> {
    let n: f64 = s.parse().map_err(|_| invalid("invalid selector number"))?;
    if n.is_finite() {
        Ok(n)
    } else {
        Err(invalid("invalid selector number"))
    }
}
fn integer(s: &str) -> io::Result<i32> {
    s.parse().map_err(|_| invalid("invalid selector integer"))
}
fn filters(args: &str) -> impl Iterator<Item = (&str, &str)> {
    args.split(',').filter_map(|a| a.split_once('='))
}
fn equal(value: &str, filter: &str) -> bool {
    match filter.strip_prefix('!') {
        Some(except) => value != except,
        None => value == filter,
    }
}
impl<'a> Selector<'a> {
    pub fn parse(s: &'a str) -> io::Result<Self> {
        if !s.starts_with('@') {
            return Ok(Self {
                kind: 0,
                args: "",
                literal: s,
            });
        }
        let kind = *s
            .as_bytes()
            .get(1)
            .ok_or(invalid("missing selector type"))?;
        if !matches!(kind, b'a' | b'e' | b'p' | b'r') {
            return Err(invalid("unknown selector type"));
        }
        let tail = &s[2..];
        let args = if tail.is_empty() {
            ""
        } else {
            tail.strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .ok_or(invalid("invalid selector brackets"))?
        };
        if !args.is_empty() {
            for f in args.split(',') {
                let (k, v) = f
                    .split_once('=')
                    .ok_or(invalid("invalid selector argument"))?;
                match k {
                    "x" | "y" | "z" | "r" | "rm" => {
                        number(v)?;
                    }
                    "c" | "m" | "l" | "lm" => {
                        integer(v)?;
                    }
                    "name" | "type" | "team" => {}
                    k if k.starts_with("score_") && k.len() > 6 => {
                        integer(v)?;
                    }
                    _ => return Err(invalid("unknown selector filter")),
                }
            }
        }
        Ok(Self {
            kind,
            args,
            literal: "",
        })
    }
    pub fn resolve(self, room: &Room, sender: &Player) -> Vec<usize> {
        if self.kind == 0 {
            return room
                .players
                .iter()
                .position(|p| p.identity.name.eq_ignore_ascii_case(self.literal))
                .into_iter()
                .collect();
        }
        let mut center = sender.pos;
        let mut count = if matches!(self.kind, b'p' | b'r') {
            1
        } else {
            0
        };
        for (k, v) in filters(self.args) {
            match k {
                "x" => center[0] = number(v).unwrap(),
                "y" => center[1] = number(v).unwrap(),
                "z" => center[2] = number(v).unwrap(),
                "c" => count = integer(v).unwrap(),
                _ => {}
            }
        }
        let mut out: Vec<_> = room
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let d = distance(p.pos, center);
                filters(self.args).all(|(k, v)| match k {
                    "r" => {
                        let r = number(v).unwrap();
                        r >= 0. && d <= r * r
                    }
                    "rm" => {
                        let r = number(v).unwrap();
                        r >= 0. && d >= r * r
                    }
                    "name" => equal(&p.identity.name, v),
                    "type" => equal("Player", v),
                    "team" => equal(&p.team, v),
                    "m" => i32::from(p.mode) == integer(v).unwrap(),
                    "l" => p.level() <= integer(v).unwrap(),
                    "lm" => p.level() >= integer(v).unwrap(),
                    k if k.starts_with("score_") => {
                        let key = &k[6..];
                        let n = integer(v).unwrap();
                        if let Some(obj) = key.strip_suffix("_min") {
                            p.scores.get(obj).is_some_and(|s| *s >= n)
                        } else {
                            p.scores.get(key).is_some_and(|s| *s <= n)
                        }
                    }
                    _ => true,
                })
            })
            .map(|(i, _)| i)
            .collect();
        if self.kind == b'r' {
            use std::hash::{BuildHasher, RandomState};
            let rng = RandomState::new();
            out.sort_unstable_by_key(|&i| rng.hash_one(room.players[i].id));
        } else {
            out.sort_unstable_by(|&a, &b| {
                distance(room.players[a].pos, center)
                    .total_cmp(&distance(room.players[b].pos, center))
                    .then_with(|| room.players[a].id.cmp(&room.players[b].id))
            });
        }
        if count < 0 {
            out.reverse();
        }
        if count != 0 {
            out.truncate(count.unsigned_abs() as usize);
        }
        out
    }
}

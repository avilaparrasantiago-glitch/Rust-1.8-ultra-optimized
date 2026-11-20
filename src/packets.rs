use crate::{
    arena::BlockPos,
    wire::{Packet, WriteWire, invalid},
};
use bytes::BufMut;
use serde_json::{Value, json};
use std::io;
use uuid::Uuid;

pub fn packed((x, y, z): BlockPos) -> i64 {
    ((i64::from(x) & 0x3ffffff) << 38) | (i64::from(y) << 26) | (i64::from(z) & 0x3ffffff)
}
pub fn unpacked(n: i64) -> io::Result<BlockPos> {
    let y = ((n >> 26) & 4095) as u16;
    Ok((
        (n >> 38) as i32,
        u8::try_from(y).map_err(|_| invalid("height outside world"))?,
        (n << 38 >> 38) as i32,
    ))
}
pub fn chat(component: &Value) -> Packet {
    Packet::new(0x02, |w| {
        w.string(&component.to_string());
        w.put_u8(0);
    })
}
pub fn text(s: impl Into<String>) -> Packet {
    chat(&json!({"text":s.into()}))
}
pub fn disconnect(id: i32, reason: &str) -> Packet {
    Packet::new(id, |w| w.string(&json!({"text":reason}).to_string()))
}
pub fn position(pos: [f64; 3], look: [f32; 2]) -> Packet {
    Packet::new(0x08, |w| {
        for n in pos {
            w.put_f64(n);
        }
        for n in look {
            w.put_f32(n);
        }
        w.put_u8(0);
    })
}
pub fn join(id: i32, mode: u8) -> Packet {
    Packet::new(0x01, |w| {
        w.put_i32(id);
        w.put_u8(mode);
        w.put_i8(0);
        w.put_u8(2);
        w.put_u8(255);
        w.string("flat");
        w.put_u8(0);
    })
}
pub fn respawn(dim: i32, mode: u8) -> Packet {
    Packet::new(0x07, |w| {
        w.put_i32(dim);
        w.put_u8(2);
        w.put_u8(mode);
        w.string("flat");
    })
}
pub fn abilities(mode: u8, flying: bool) -> Packet {
    Packet::new(0x39, |w| {
        let flags = match mode {
            1 => 13 | ((flying as u8) << 1),
            3 => 7,
            _ => 0,
        };
        w.put_u8(flags);
        w.put_f32(0.05);
        w.put_f32(0.1);
    })
}
pub fn game_state(reason: u8, value: f32) -> Packet {
    Packet::new(0x2b, |w| {
        w.put_u8(reason);
        w.put_f32(value);
    })
}
pub fn health(hp: f32, food: i32, sat: f32) -> Packet {
    Packet::new(0x06, |w| {
        w.put_f32(hp);
        w.var(food);
        w.put_f32(sat);
    })
}
pub fn spawn_point(pos: [f64; 3]) -> Packet {
    Packet::new(0x05, |w| {
        w.put_i64(packed((
            pos[0].floor() as i32,
            pos[1] as u8,
            pos[2].floor() as i32,
        )))
    })
}
pub fn block(p: BlockPos, state: u16) -> Packet {
    Packet::new(0x23, |w| {
        w.put_i64(packed(p));
        w.var(i32::from(state));
    })
}
pub fn unload((cx, cz): (i32, i32)) -> Packet {
    Packet::new(0x21, |w| {
        w.put_i32(cx);
        w.put_i32(cz);
        w.put_u8(1);
        w.put_u16(0);
        w.var(0);
    })
}
pub fn sign(p: BlockPos, lines: &[Value; 4]) -> Packet {
    Packet::new(0x33, |w| {
        w.put_i64(packed(p));
        for line in lines {
            w.string(&line.to_string());
        }
    })
}
pub fn effect(id: i32, p: BlockPos, data: i32) -> Packet {
    Packet::new(0x28, |w| {
        w.put_i32(id);
        w.put_i64(packed(p));
        w.put_i32(data);
        w.put_u8(0);
    })
}
pub fn particle(id: i32, pos: [f64; 3], count: i32, extra: &[i32]) -> io::Result<Packet> {
    let n = match id {
        36 => 2,
        37 | 38 => 1,
        0..=41 => 0,
        _ => return Err(invalid("unknown particle")),
    };
    if n != extra.len() || !(0..=1024).contains(&count) {
        return Err(invalid("invalid particle arguments"));
    }
    Ok(Packet::new(0x2a, |w| {
        w.put_i32(id);
        w.put_u8(0);
        for v in pos {
            w.put_f32(v as f32);
        }
        for v in [0.2, 0.2, 0.2, 0.0] {
            w.put_f32(v);
        }
        w.put_i32(count);
        for &v in extra {
            w.var(v);
        }
    }))
}
pub fn stats(entries: &[(&str, i32)]) -> Packet {
    Packet::new(0x37, |w| {
        w.var(entries.len() as i32);
        for (k, v) in entries {
            w.string(k);
            w.var(*v);
        }
    })
}
pub fn completions(matches: &[String]) -> Packet {
    Packet::new(0x3a, |w| {
        w.var(matches.len() as i32);
        for s in matches {
            w.string(s);
        }
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stack {
    pub id: i16,
    pub count: u8,
    pub damage: i16,
}
pub fn slot(window: i8, index: i16, item: Option<Stack>) -> Packet {
    Packet::new(0x2f, |w| {
        w.put_i8(window);
        w.put_i16(index);
        if let Some(i) = item {
            w.put_i16(i.id);
            w.put_u8(i.count);
            w.put_i16(i.damage);
            w.put_u8(0);
        } else {
            w.put_i16(-1);
        }
    })
}
pub struct MapPatch<'a> {
    pub x: u8,
    pub y: u8,
    pub width: u8,
    pub height: u8,
    pub pixels: &'a [u8],
}
pub fn map(
    id: i32,
    scale: u8,
    icons: &[[u8; 3]],
    patch: Option<MapPatch<'_>>,
) -> io::Result<Packet> {
    if scale > 4 || icons.len() > 256 {
        return Err(invalid("invalid map header"));
    }
    if let Some(p) = &patch
        && (p.width == 0
            || p.height == 0
            || usize::from(p.x) + usize::from(p.width) > 128
            || usize::from(p.y) + usize::from(p.height) > 128
            || p.pixels.len() != usize::from(p.width) * usize::from(p.height))
    {
        return Err(invalid("invalid map rectangle"));
    }
    Ok(Packet::new(0x34, |w| {
        w.var(id);
        w.put_u8(scale);
        w.var(icons.len() as i32);
        for icon in icons {
            w.extend_from_slice(icon);
        }
        if let Some(p) = patch {
            w.extend_from_slice(&[p.width, p.height, p.x, p.y]);
            w.var(p.pixels.len() as i32);
            w.extend_from_slice(p.pixels);
        } else {
            w.put_u8(0);
        }
    }))
}
pub fn list_add(uuid: Uuid, name: &str, mode: u8) -> Packet {
    Packet::new(0x38, |w| {
        w.var(0);
        w.var(1);
        w.extend_from_slice(uuid.as_bytes());
        w.string(name);
        w.var(0);
        w.var(i32::from(mode));
        w.var(0);
        w.put_u8(0);
    })
}
pub fn list_remove(uuid: Uuid) -> Packet {
    Packet::new(0x38, |w| {
        w.var(4);
        w.var(1);
        w.extend_from_slice(uuid.as_bytes());
    })
}
fn angle(a: f32) -> u8 {
    (a.rem_euclid(360.) * (256. / 360.)) as u8
}
pub fn spawn_player(id: i32, uuid: Uuid, pos: [f64; 3], look: [f32; 2]) -> Packet {
    Packet::new(0x0c, |w| {
        w.var(id);
        w.extend_from_slice(uuid.as_bytes());
        for n in pos {
            w.put_i32((n * 32.).floor() as i32);
        }
        w.put_u8(angle(look[0]));
        w.put_u8(angle(look[1]));
        w.put_i16(0);
        w.put_u8(0x7f);
    })
}
pub fn entity_pos(id: i32, pos: [f64; 3], look: [f32; 2], ground: bool) -> Packet {
    Packet::new(0x18, |w| {
        w.var(id);
        for n in pos {
            w.put_i32((n * 32.).floor() as i32);
        }
        w.put_u8(angle(look[0]));
        w.put_u8(angle(look[1]));
        w.put_u8(ground as u8);
    })
}
pub fn head(id: i32, yaw: f32) -> Packet {
    Packet::new(0x19, |w| {
        w.var(id);
        w.put_u8(angle(yaw));
    })
}
pub fn destroy(id: i32) -> Packet {
    Packet::new(0x13, |w| {
        w.var(1);
        w.var(id);
    })
}
pub fn velocity(id: i32, v: [f64; 3]) -> Packet {
    Packet::new(0x12, |w| {
        w.var(id);
        for n in v {
            w.put_i16((n.clamp(-3.9, 3.9) * 8000.) as i16);
        }
    })
}

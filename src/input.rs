use crate::{
    arena::BlockPos,
    packets::{Stack, unpacked},
    wire::{Cursor, invalid},
};
use std::io;

#[derive(Debug)]
pub enum Action {
    KeepAlive(i32),
    Chat(String),
    Complete(String),
    Stats,
    Respawn,
    Move {
        pos: Option<[f64; 3]>,
        look: Option<[f32; 2]>,
        ground: bool,
    },
    Dig(u8, BlockPos),
    Place(BlockPos, u8),
    UseHeld,
    Held(u8),
    Ability(bool),
    Attack(i32),
    Sprint(bool),
    Swing,
    Close(u8),
    Creative(i16, Option<Stack>),
    Click {
        window: u8,
        slot: i16,
        button: u8,
        sequence: i16,
        mode: u8,
    },
    Sign(BlockPos, [String; 4]),
    Ignore,
}

fn nbt_string(r: &mut Cursor<'_>) -> io::Result<()> {
    let n = r.short()? as u16;
    r.take(n as usize)?;
    Ok(())
}
fn nbt(r: &mut Cursor<'_>, tag: u8, depth: u8, remaining: &mut usize) -> io::Result<()> {
    if depth > 64 || *remaining == 0 {
        return Err(invalid("NBT budget exceeded"));
    }
    *remaining -= 1;
    match tag {
        1..=6 => {
            let sizes = [1, 2, 4, 8, 4, 8];
            r.take(sizes[usize::from(tag - 1)])?;
        }
        7 | 11 => {
            let n = usize::try_from(r.int()?).map_err(|_| invalid("negative NBT array"))?;
            let len = n
                .checked_mul(if tag == 7 { 1 } else { 4 })
                .ok_or(invalid("NBT array overflow"))?;
            r.take(len)?;
        }
        8 => nbt_string(r)?,
        9 => {
            let child = r.byte()?;
            let n = r.int()?;
            if n < 0 || n as usize > *remaining || child > 11 || (child == 0 && n != 0) {
                return Err(invalid("invalid NBT list"));
            }
            for _ in 0..n {
                nbt(r, child, depth + 1, remaining)?;
            }
        }
        10 => loop {
            let child = r.byte()?;
            if child == 0 {
                break;
            }
            nbt_string(r)?;
            nbt(r, child, depth + 1, remaining)?;
        },
        _ => return Err(invalid("unknown NBT type")),
    }
    Ok(())
}
fn stack(r: &mut Cursor<'_>) -> io::Result<Option<Stack>> {
    let id = r.short()?;
    if id == -1 {
        return Ok(None);
    }
    let count = r.byte()?;
    let damage = r.short()?;
    if id <= 0 || !(1..=64).contains(&count) {
        return Err(invalid("invalid item stack"));
    }
    match r.byte()? {
        0 => {}
        10 => {
            nbt_string(r)?;
            let mut budget = 32768;
            nbt(r, 10, 0, &mut budget)?;
        }
        _ => return Err(invalid("invalid slot NBT")),
    }
    Ok(Some(Stack { id, count, damage }))
}

pub fn decode(data: &[u8]) -> io::Result<Action> {
    let mut r = Cursor(data);
    let action = match r.var()? {
        0x00 => Action::KeepAlive(r.var()?),
        0x01 => Action::Chat(r.text(100)?.to_owned()),
        0x02 => {
            let id = r.var()?;
            match r.var()? {
                0 => Action::Ignore,
                1 => Action::Attack(id),
                2 => {
                    for _ in 0..3 {
                        r.float()?;
                    }
                    Action::Ignore
                }
                _ => return Err(invalid("invalid use entity")),
            }
        }
        id @ 0x03..=0x06 => {
            let pos = if matches!(id, 4 | 6) {
                Some([r.coord()?, r.coord()?, r.coord()?])
            } else {
                None
            };
            let look = if matches!(id, 5 | 6) {
                Some([r.float()?.rem_euclid(360.), r.float()?.clamp(-90., 90.)])
            } else {
                None
            };
            Action::Move {
                pos,
                look,
                ground: r.bool()?,
            }
        }
        0x07 => {
            let status = r.byte()?;
            let pos = unpacked(r.long()?)?;
            let face = r.byte()?;
            if status > 5 || face > 5 && face != 255 {
                return Err(invalid("invalid dig"));
            }
            Action::Dig(status, pos)
        }
        0x08 => {
            let raw = r.long()?;
            let face = r.byte()?;
            stack(&mut r)?;
            r.take(3)?;
            if face == 255 {
                Action::UseHeld
            } else if face <= 5 {
                Action::Place(unpacked(raw)?, face)
            } else {
                return Err(invalid("invalid placement face"));
            }
        }
        0x09 => {
            let slot = r.short()?;
            if !(0..=8).contains(&slot) {
                return Err(invalid("invalid held slot"));
            }
            Action::Held(slot as u8)
        }
        0x0a => Action::Swing,
        0x0b => {
            r.var()?;
            let action = r.var()?;
            r.var()?;
            match action {
                3 => Action::Sprint(true),
                4 => Action::Sprint(false),
                0..=6 => Action::Ignore,
                _ => return Err(invalid("invalid action")),
            }
        }
        0x0c => {
            r.float()?;
            r.float()?;
            r.byte()?;
            Action::Ignore
        }
        0x0d => Action::Close(r.byte()?),
        0x0e => {
            let window = r.byte()?;
            let slot = r.short()?;
            let button = r.byte()?;
            let sequence = r.short()?;
            let mode = r.byte()?;
            stack(&mut r)?;
            if !(slot == -999 || (0..=89).contains(&slot)) || mode > 6 {
                return Err(invalid("invalid inventory click"));
            }
            Action::Click {
                window,
                slot,
                button,
                sequence,
                mode,
            }
        }
        0x0f => {
            r.byte()?;
            r.short()?;
            r.bool()?;
            Action::Ignore
        }
        0x10 => {
            let slot = r.short()?;
            if !(-1..=44).contains(&slot) {
                return Err(invalid("invalid creative slot"));
            }
            Action::Creative(slot, stack(&mut r)?)
        }
        0x11 => {
            r.byte()?;
            r.byte()?;
            Action::Ignore
        }
        0x12 => {
            let pos = unpacked(r.long()?)?;
            let mut lines = std::array::from_fn(|_| String::new());
            for line in &mut lines {
                *line = r.text(384)?.to_owned();
            }
            Action::Sign(pos, lines)
        }
        0x13 => {
            let flags = r.byte()?;
            r.float()?;
            r.float()?;
            Action::Ability(flags & 2 != 0)
        }
        0x14 => {
            let s = r.text(32767)?;
            if r.bool()? {
                r.long()?;
            }
            Action::Complete(s.to_owned())
        }
        0x15 => {
            r.text(7)?;
            r.byte()?;
            r.byte()?;
            r.bool()?;
            r.byte()?;
            Action::Ignore
        }
        0x16 => match r.var()? {
            0 => Action::Respawn,
            1 => Action::Stats,
            2 => Action::Ignore,
            _ => return Err(invalid("invalid client status")),
        },
        0x17 => {
            let channel = r.text(20)?;
            if r.0.len() > 32767 {
                return Err(invalid("plugin payload too large"));
            }
            if matches!(channel, "MC|BEdit" | "MC|BSign") {
                stack(&mut r)?;
            } else {
                r.take(r.0.len())?;
            }
            Action::Ignore
        }
        0x18 => {
            r.take(16)?;
            Action::Ignore
        }
        0x19 => {
            r.text(40)?;
            if !(0..=3).contains(&r.var()?) {
                return Err(invalid("invalid resource pack result"));
            }
            Action::Ignore
        }
        _ => return Err(invalid("unknown serverbound play packet")),
    };
    r.end()?;
    Ok(action)
}

use crate::packets::Stack;
use crossbeam_channel::{Receiver, Sender};
use mlua::{ChunkMode, Function, Lua, LuaOptions, StdLib, Table, VmState};
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering::{Acquire, Release},
        },
    },
    time::{Duration, Instant},
};

pub struct Event {
    pub kind: &'static str,
    pub player: i32,
    pub name: String,
    pub pos: [f64; 3],
    pub block: u16,
}
pub struct Reply {
    pub player: i32,
    pub message: Option<String>,
    pub broadcast: Option<String>,
    pub kit: Option<Stack>,
    pub mode: Option<u8>,
}
struct Game {
    lua: Lua,
    handler: Function,
}

fn cap(s: Option<String>) -> mlua::Result<Option<String>> {
    if s.as_ref().is_some_and(|s| s.len() > 512) {
        return Err(mlua::Error::runtime("message too long"));
    }
    Ok(s)
}
impl Game {
    fn load(source: &str) -> mlua::Result<Self> {
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH,
            LuaOptions::default(),
        )?;
        lua.set_memory_limit(4 << 20)?;
        for key in [
            "require",
            "loadstring",
            "load",
            "dofile",
            "loadfile",
            "collectgarbage",
            "getfenv",
            "setfenv",
            "print",
            "pcall",
            "xpcall",
        ] {
            lua.globals().set(key, mlua::Value::Nil)?;
        }
        lua.sandbox(true)?;
        let start = Instant::now();
        lua.set_interrupt(move |_| {
            if start.elapsed() > Duration::from_millis(30) {
                Err(mlua::Error::runtime(
                    "script initialization budget exceeded",
                ))
            } else {
                Ok(VmState::Continue)
            }
        });
        let handler: Function = lua
            .load(source)
            .set_name("minigame")
            .set_mode(ChunkMode::Text)
            .eval()?;
        lua.remove_interrupt();
        Ok(Self { lua, handler })
    }
    fn event(&self, event: Event) -> mlua::Result<Option<Reply>> {
        let start = Instant::now();
        self.lua.set_interrupt(move |_| {
            if start.elapsed() > Duration::from_millis(3) {
                Err(mlua::Error::runtime("script event budget exceeded"))
            } else {
                Ok(VmState::Continue)
            }
        });
        let args = self.lua.create_table()?;
        args.set("kind", event.kind)?;
        args.set("name", event.name)?;
        args.set("entity", event.player)?;
        args.set("x", event.pos[0])?;
        args.set("y", event.pos[1])?;
        args.set("z", event.pos[2])?;
        args.set("block", event.block)?;
        let Some(out) = self.handler.call::<Option<Table>>(args)? else {
            return Ok(None);
        };
        let kit = if let Some(item) = out.raw_get::<Option<i16>>("item")? {
            let count = out.raw_get::<Option<u8>>("count")?.unwrap_or(1);
            if !(1..=431).contains(&item) || !(1..=64).contains(&count) {
                return Err(mlua::Error::runtime("invalid kit stack"));
            }
            Some(Stack {
                id: item,
                count,
                damage: 0,
            })
        } else {
            None
        };
        let mode = out.raw_get::<Option<u8>>("gamemode")?;
        if mode.is_some_and(|m| m > 3) {
            return Err(mlua::Error::runtime("invalid game mode"));
        }
        Ok(Some(Reply {
            player: event.player,
            message: cap(out.raw_get("message")?)?,
            broadcast: cap(out.raw_get("broadcast")?)?,
            kit,
            mode,
        }))
    }
}
fn source(path: &Path) -> io::Result<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(131073)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 131072 {
        return Err(io::Error::other("script exceeds 128 KiB"));
    }
    String::from_utf8(bytes).map_err(io::Error::other)
}
pub struct Scripts {
    pub events: Sender<Event>,
    pub replies: Receiver<Reply>,
    stop: Arc<AtomicBool>,
}
impl Scripts {
    pub fn start(path: PathBuf) -> io::Result<Self> {
        let initial = source(&path)?;
        let mut game = Some(Game::load(&initial).map_err(io::Error::other)?);
        let (events, rx) = crossbeam_channel::bounded(256);
        let (tx, replies) = crossbeam_channel::bounded(256);
        let stop = Arc::new(AtomicBool::new(false));
        let closed = stop.clone();
        std::thread::Builder::new()
            .name("luau-events".into())
            .spawn(move || {
                let mut last = initial;
                let mut check = Instant::now();
                while !closed.load(Acquire) {
                    if check.elapsed() >= Duration::from_millis(500) {
                        check = Instant::now();
                        if let Ok(next) = source(&path)
                            && next != last
                        {
                            match Game::load(&next) {
                                Ok(g) => game = Some(g),
                                Err(e) => eprintln!("Luau reload rejected: {e}"),
                            }
                            last = next;
                        }
                    }
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(event) => {
                            if let Some(g) = &game {
                                match g.event(event) {
                                    Ok(Some(reply)) => {
                                        let _ = tx.try_send(reply);
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        eprintln!("Luau listener disabled: {e}");
                                        game = None;
                                    }
                                }
                            }
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                        Err(_) => break,
                    }
                }
            })?;
        Ok(Self {
            events,
            replies,
            stop,
        })
    }
}
impl Drop for Scripts {
    fn drop(&mut self) {
        self.stop.store(true, Release);
    }
}

use bytes::{Buf, BufMut, Bytes, BytesMut};
use libdeflater::{CompressionLvl, Compressor, Decompressor};
use std::{
    cell::RefCell,
    io,
    sync::{Arc, OnceLock},
};

pub const MAX_FRAME: usize = (1 << 21) - 1;
pub const MAX_BODY: usize = 2 << 20;
pub fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn prefix(data: &[u8], width: usize) -> io::Result<Option<(i32, usize)>> {
    let mut n = 0u32;
    for (i, &b) in data.iter().take(width).enumerate() {
        if i == 4 && b > 15 {
            return Err(invalid("VarInt overflow"));
        }
        n |= u32::from(b & 127) << (7 * i);
        if b & 128 == 0 {
            return Ok(Some((n as i32, i + 1)));
        }
    }
    if data.len() >= width {
        Err(invalid("unterminated VarInt"))
    } else {
        Ok(None)
    }
}

pub trait WriteWire: BufMut {
    fn var(&mut self, n: i32) {
        let mut v = n as u32;
        while v >= 128 {
            self.put_u8(v as u8 | 128);
            v >>= 7;
        }
        self.put_u8(v as u8);
    }
    fn string(&mut self, text: &str) {
        self.var(text.len() as i32);
        self.put_slice(text.as_bytes());
    }
}
impl<T: BufMut> WriteWire for T {}

pub struct Cursor<'a>(pub &'a [u8]);
impl<'a> Cursor<'a> {
    pub fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        let (head, tail) = self
            .0
            .split_at_checked(n)
            .ok_or(invalid("truncated packet"))?;
        self.0 = tail;
        Ok(head)
    }
    pub fn byte(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> io::Result<bool> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid("invalid boolean")),
        }
    }
    pub fn short(&mut self) -> io::Result<i16> {
        Ok(i16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn int(&mut self) -> io::Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn long(&mut self) -> io::Result<i64> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn float(&mut self) -> io::Result<f32> {
        let n = f32::from_bits(self.int()? as u32);
        if n.is_finite() {
            Ok(n)
        } else {
            Err(invalid("non-finite angle"))
        }
    }
    pub fn coord(&mut self) -> io::Result<f64> {
        let n = f64::from_bits(self.long()? as u64);
        if n.abs() <= 30_000_000. {
            Ok(n)
        } else {
            Err(invalid("invalid coordinate"))
        }
    }
    pub fn var(&mut self) -> io::Result<i32> {
        let (n, used) = prefix(self.0, 5)?.ok_or(invalid("truncated VarInt"))?;
        self.0 = &self.0[used..];
        Ok(n)
    }
    pub fn text(&mut self, units: usize) -> io::Result<&'a str> {
        let len = self.var()? as usize;
        if len > units * 4 {
            return Err(invalid("string exceeds byte limit"));
        }
        let s = std::str::from_utf8(self.take(len)?).map_err(|_| invalid("invalid UTF-8"))?;
        if s.encode_utf16().count() > units {
            return Err(invalid("string exceeds character limit"));
        }
        Ok(s)
    }
    pub fn end(&self) -> io::Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(invalid("trailing packet data"))
        }
    }
}

thread_local! {
    static DEFLATE: RefCell<Compressor> = RefCell::new(Compressor::new(CompressionLvl::default()));
    static INFLATE: RefCell<Decompressor> = RefCell::new(Decompressor::new());
    static SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

pub fn split_frame(src: &mut BytesMut) -> io::Result<Option<Bytes>> {
    let Some((len, used)) = prefix(src, 3)? else {
        return Ok(None);
    };
    if len == 0 {
        return Err(invalid("empty frame"));
    }
    if src.len() < used + len as usize {
        return Ok(None);
    }
    src.advance(used);
    Ok(Some(src.split_to(len as usize).freeze()))
}

pub fn unpack(mut frame: Bytes, threshold: Option<usize>) -> io::Result<Bytes> {
    let Some(t) = threshold else { return Ok(frame) };
    let mut r = Cursor(&frame);
    let declared = r.var()? as usize;
    let skip = frame.len() - r.0.len();
    frame.advance(skip);
    if declared == 0 {
        if frame.is_empty() || frame.len() >= t {
            return Err(invalid("invalid raw envelope"));
        }
        return Ok(frame);
    }
    if declared < t || declared > MAX_BODY {
        return Err(invalid("invalid inflated size"));
    }
    let mut out = vec![0; declared];
    let written = INFLATE
        .with_borrow_mut(|d| d.zlib_decompress(&frame, &mut out))
        .map_err(|_| invalid("invalid zlib stream"))?;
    if written != declared {
        return Err(invalid("inflated length mismatch"));
    }
    Ok(out.into())
}

pub fn frame(body: &[u8], threshold: Option<usize>) -> io::Result<Bytes> {
    if body.is_empty() || body.len() > MAX_BODY {
        return Err(invalid("invalid outbound size"));
    }
    let compressed = threshold.is_some_and(|t| body.len() >= t);
    SCRATCH.with_borrow_mut(|buf| {
        let mut header = BytesMut::with_capacity(5);
        let data = if compressed {
            header.var(body.len() as i32);
            DEFLATE.with_borrow_mut(|c| {
                buf.resize(c.zlib_compress_bound(body.len()), 0);
                let n = c.zlib_compress(body, buf).map_err(io::Error::other)?;
                buf.truncate(n);
                Ok::<_, io::Error>(())
            })?;
            &buf[..]
        } else {
            if threshold.is_some() {
                header.put_u8(0);
            }
            body
        };
        let len = header.len() + data.len();
        if len > MAX_FRAME {
            return Err(invalid("frame exceeds VarInt21"));
        }
        let mut out = BytesMut::with_capacity(3 + len);
        out.var(len as i32);
        out.extend_from_slice(&header);
        out.extend_from_slice(data);
        Ok(out.freeze())
    })
}

/// Created once by the simulation. Each negotiated threshold gets one immutable frame.
#[derive(Clone)]
pub struct Packet(Arc<Shared>);
struct Shared {
    body: Bytes,
    encoded: [OnceLock<Bytes>; 2],
}
impl Packet {
    pub fn new(id: i32, fill: impl FnOnce(&mut BytesMut)) -> Self {
        Self::with_capacity(id, 48, fill)
    }
    pub(crate) fn with_capacity(
        id: i32,
        capacity: usize,
        fill: impl FnOnce(&mut BytesMut),
    ) -> Self {
        let mut body = BytesMut::with_capacity(capacity);
        body.var(id);
        fill(&mut body);
        Self(Arc::new(Shared {
            body: body.freeze(),
            encoded: Default::default(),
        }))
    }
    pub fn body(&self) -> &[u8] {
        &self.0.body
    }
    pub fn encoded(&self, threshold: usize) -> io::Result<Bytes> {
        let idx = match threshold {
            256 => 0,
            512 => usize::from((256..512).contains(&self.body().len())),
            _ => return Err(invalid("unknown compression profile")),
        };
        // All packet constructors enforce bounds. Cache successes and errors without racing compression.
        let cell = &self.0.encoded[idx];
        let b = cell.get_or_init(|| frame(self.body(), Some(threshold)).unwrap_or_default());
        if b.is_empty() {
            Err(invalid("outbound frame rejected"))
        } else {
            Ok(b.clone())
        }
    }
}

#[cfg(test)]
mod packet_cache_tests {
    use super::*;

    #[test]
    fn chunk_profiles_share_the_same_compressed_allocation() -> io::Result<()> {
        let packet = Packet::new(0x21, |w| w.resize(1024, 0));
        let low = packet.encoded(256)?;
        let high = packet.encoded(512)?;
        assert_eq!(low.as_ptr(), high.as_ptr());
        let medium = Packet::new(0x21, |w| w.resize(300, 1));
        assert_ne!(medium.encoded(256)?, medium.encoded(512)?);
        Ok(())
    }
}

use crate::wire::invalid;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{
    collections::HashMap,
    io,
    net::IpAddr,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub struct Forwarding {
    key: Vec<u8>,
    seen: Mutex<HashMap<Uuid, u64>>,
}
pub struct Identity {
    pub uuid: Uuid,
    pub name: String,
    pub ip: IpAddr,
}
pub fn username(s: &str) -> bool {
    (1..=16).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
pub fn offline(name: &str, ip: IpAddr) -> Identity {
    let b = uuid::Builder::from_md5_bytes({
        // Java's nameUUIDFromBytes hashes the raw OfflinePlayer prefix without a UUID namespace.
        use md5::{Digest, Md5};
        let mut h = Md5::new();
        h.update(b"OfflinePlayer:");
        h.update(name.as_bytes());
        h.finalize().into()
    });
    Identity {
        uuid: b.into_uuid(),
        name: name.into(),
        ip,
    }
}
impl Forwarding {
    pub fn new(key: Vec<u8>) -> io::Result<Self> {
        if key.len() < 32 {
            return Err(invalid("forwarding key must be at least 32 bytes"));
        }
        Ok(Self {
            key,
            seen: Mutex::new(HashMap::new()),
        })
    }
    pub fn verify(&self, host: &str, login: &str) -> io::Result<Identity> {
        let (claims, sig) = host
            .rsplit_once('\0')
            .ok_or(invalid("missing proxy signature"))?;
        if sig.len() != 64 || !sig.is_ascii() {
            return Err(invalid("invalid signature length"));
        }
        let mut tag = [0u8; 32];
        for (i, v) in tag.iter_mut().enumerate() {
            *v = u8::from_str_radix(&sig[i * 2..i * 2 + 2], 16)
                .map_err(|_| invalid("invalid signature"))?;
        }
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&self.key).map_err(|_| invalid("invalid key"))?;
        mac.update(claims.as_bytes());
        mac.verify_slice(&tag)
            .map_err(|_| invalid("untrusted forwarding data"))?;
        let mut parts = claims.split('\0');
        let mut next = || parts.next().ok_or(invalid("incomplete forwarding claims"));
        if next()? != "Coral47/1" || next()? != login {
            return Err(invalid("identity mismatch"));
        }
        let uuid = Uuid::parse_str(next()?).map_err(|_| invalid("invalid UUID"))?;
        let ip = next()?
            .parse()
            .map_err(|_| invalid("invalid forwarded address"))?;
        let issued: u64 = next()?.parse().map_err(|_| invalid("invalid timestamp"))?;
        let nonce = Uuid::parse_str(next()?).map_err(|_| invalid("invalid nonce"))?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_secs();
        if parts.next().is_some() || issued.abs_diff(now) > 15 {
            return Err(invalid("expired forwarding claims"));
        }
        let mut seen = self.seen.lock().unwrap();
        if seen.len() >= 65536 {
            seen.retain(|_, until| *until >= now);
        }
        if seen.get(&nonce).is_some_and(|until| *until >= now) || seen.len() >= 65536 {
            return Err(invalid("duplicate or excess forwarding claims"));
        }
        seen.insert(nonce, issued.saturating_add(15));
        Ok(Identity {
            uuid,
            name: login.into(),
            ip,
        })
    }
}

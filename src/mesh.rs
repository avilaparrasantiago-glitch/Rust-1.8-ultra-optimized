use crate::{engine::Manager, packets, wire::invalid};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{
    io,
    sync::{
        Arc,
        atomic::Ordering::{Acquire, Relaxed},
    },
    time::Duration,
};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
struct Chat {
    node: Uuid,
    text: String,
}
fn env(key: &str) -> io::Result<String> {
    std::env::var(key).map_err(|_| invalid("connector environment variable missing"))
}
pub async fn start(manager: Arc<Manager>) -> io::Result<()> {
    let node = manager.node_id;
    if let Some(key) = &manager.cfg.nats_url_env {
        let bus = async_nats::connect(env(key)?)
            .await
            .map_err(io::Error::other)?;
        let mut sub = bus
            .subscribe("coral.v1.chat")
            .await
            .map_err(io::Error::other)?;
        bus.flush().await.map_err(io::Error::other)?;
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1024);
        let _ = manager.mesh.set(tx);
        let mgr = manager.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(text)=rx.recv()=>{if let Ok(body)=serde_json::to_vec(&Chat{node,text}) && let Err(e)=bus.publish("coral.v1.chat",body.into()).await{eprintln!("Global chat publish failed: {e}");}}
                    value=sub.next()=>{let Some(m)=value else{break;};if m.payload.len()<=4096 && let Ok(c)=serde_json::from_slice::<Chat>(&m.payload) && c.node!=node && c.text.len()<=1024{mgr.announce(packets::text(c.text));}}
                }
                if mgr.stopping.load(Acquire) {
                    break;
                }
            }
        });
    }
    if let Some(key) = &manager.cfg.redis_url_env {
        let client = redis::Client::open(env(key)?).map_err(io::Error::other)?;
        let mut conn = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(io::Error::other)?;
        let mgr = manager.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            let key = format!("coral:v1:nodes:{node}");
            loop {
                interval.tick().await;
                if mgr.stopping.load(Acquire) {
                    break;
                }
                let state = serde_json::json!({"node":node,"players":mgr.metrics.players.load(Relaxed),"capacity":mgr.cfg.max_players,"worlds":mgr.names()});
                let result: redis::RedisResult<()> = redis::cmd("SET")
                    .arg(&key)
                    .arg(state.to_string())
                    .arg("EX")
                    .arg(8)
                    .query_async(&mut conn)
                    .await;
                if let Err(e) = result {
                    eprintln!("Heartbeat error: {e}");
                }
            }
        });
    }
    Ok(())
}

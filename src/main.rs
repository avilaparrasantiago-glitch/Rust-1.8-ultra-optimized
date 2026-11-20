use syrust_engine::{
    arena::Template,
    config::{Config, Terrain, persistent_seed},
    engine::Manager,
    identity::Forwarding,
    network,
    wire::invalid,
};
use std::{io, path::Path};

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let [action, path] = args.as_slice()
        && action == "arena-create"
    {
        return Template::flat(4).save(Path::new(path));
    }
    if let [action, src, dst] = args.as_slice() {
        if action == "region-import" {
            return syrust_engine::anvil::import(Path::new(src))?.save(Path::new(dst));
        }
        if action == "region-export" {
            std::fs::create_dir(dst)?;
            return syrust_engine::anvil::export(
                Path::new(dst),
                &Template::load(Path::new(src))?,
                &Default::default(),
            );
        }
    }
    if args.as_slice() == ["config"] {
        println!("{}", serde_json::to_string_pretty(&Config::default())?);
        return Ok(());
    }
    let file = match args.as_slice() {
        [] => None,
        [a, path] if a == "--config" => Some(Path::new(path)),
        _ => {
            return Err(invalid(
                "usage: syrust [--config FILE | config | arena-create FILE]",
            ));
        }
    };
    let cfg = Config::load(file)?;
    let secret = cfg
        .proxy_secret_env
        .as_ref()
        .map(|name| {
            let key = std::env::var(name)
                .map_err(|_| invalid("missing proxy key environment variable"))?;
            Forwarding::new(key.into_bytes())
        })
        .transpose()?;
    let seed = persistent_seed(&cfg.data_dir)?;
    let radius = i32::from(cfg.view_distance) + 1;
    let template = match &cfg.arena_file {
        Some(p) => Template::load(p)?,
        None => match cfg.terrain {
            Terrain::Flat => Template::flat(radius),
            Terrain::Normal => Template::normal(seed, radius),
        },
    };
    let manager = Manager::new(cfg, template, seed)?;
    let run = async {
        let url = manager
            .cfg
            .database_url_env
            .as_ref()
            .map(|k| std::env::var(k).map_err(|_| invalid("missing database URL")))
            .transpose()?;
        let profiles = syrust_engine::profiles::Profiles::open(
            &manager.cfg.data_dir,
            url,
            manager.cfg.max_players,
        )
        .await?;
        syrust_engine::mesh::start(manager.clone()).await?;
        let result = tokio::select! {r=network::serve(manager.clone(),profiles.clone(),secret)=>r,r=tokio::signal::ctrl_c()=>r};
        manager.shutdown();
        profiles.close().await?;
        result
    };
    #[cfg(target_os = "linux")]
    {
        tokio_uring::start(run)
    }
    #[cfg(not(target_os = "linux"))]
    {
        tokio::task::LocalSet::new().block_on(
            &tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?,
            run,
        )
    }
}

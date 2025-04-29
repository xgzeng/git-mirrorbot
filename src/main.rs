mod config;
mod repo;

use anyhow::{anyhow, Context, Result};

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[arg(short, long, default_value = "config.yml")]
    config: PathBuf,
    #[arg(short, long)]
    storage_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    // init log
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    // parse args
    let cli = Cli::parse();
    log::info!("Use config file at {:?}", cli.config);
    let mut app_config = config::from_file(&cli.config).context("parse config file")?;
    if let Some(storage_dir) = cli.storage_dir {
        app_config.storage_dir = storage_dir;
    }
    log::info!("Storage dir: {:?}", app_config.storage_dir);
    if !app_config.storage_dir.exists() {
        return Err(anyhow!(
            "Storage dir not exists: {:?}",
            app_config.storage_dir
        ));
    }

    for repo_name in &app_config.mirrors {
        if let Err(err) = repo::sync_with_progressbar(repo_name, &app_config.storage_dir) {
            log::error!("sync {} error: {:?}", repo_name, err);
        }
    }

    Ok(())
}

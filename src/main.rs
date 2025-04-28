mod config;
mod repo;

use anyhow::{Context, Result};

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[arg(short, long, default_value = "config.yml")]
    config: PathBuf,
}

fn main() -> Result<()> {
    // init log
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    // parse args
    let cli = Cli::parse();
    log::info!("config: {:?}", cli.config);

    let app_config = config::from_file(&cli.config).context("parse config file")?;

    for repo_name in &app_config.mirrors {
        if let Err(err) = repo::sync_with_progressbar(repo_name, &app_config.storage_dir) {
            log::error!("sync {} error: {:?}", repo_name, err);
        }
    }

    Ok(())
}

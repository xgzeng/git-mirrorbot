mod repo;

use anyhow::Context;
use repo::{RepoConfig, RepoManager};
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
struct MainConfig {
    github_repos: Vec<String>,

    #[serde(default)]
    repo: Vec<RepoConfig>,
}

fn main() {
    // init log
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    // read config
    let config_str = std::fs::read_to_string("git-mirror.toml").expect("read config file failed");

    let config: MainConfig = toml::from_str(config_str.as_str())
        .context("parse config file")
        .unwrap();

    log::info!("{:?}", config);

    let mut repo_manager = RepoManager::new();
    for repo_name in &config.github_repos {
        repo_manager
            .add_github_repo(repo_name)
            .expect("add repo failed")
    }

    for r in &config.repo {
        repo_manager.add_repo(r.clone()).expect("add repo failed")
    }

    repo_manager.update();
}

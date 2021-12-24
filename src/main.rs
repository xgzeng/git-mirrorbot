mod repo;

use anyhow::Context;
use repo::MirrorBot;
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
struct MainConfig {
    github_repos: Vec<String>,
    // #[serde(default)]
    // repo: Vec<RepoConfig>,
}

fn main() {
    // init log
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    // let repo_names = list_github_user_repos("xgzeng").expect("list_github_user_repos");
    // println!("{} repos got", repo_names.len());

    // read config file
    let config_str = std::fs::read_to_string("git-mirror.toml").expect("read config file failed");

    let config: MainConfig = toml::from_str(config_str.as_str())
        .context("parse config file")
        .unwrap();

    log::info!("{:?}", config);

    for repo_name in &config.github_repos {
        let mirror = MirrorBot::from_simple_name(repo_name).expect("");
        if let Err(err) = mirror.sync_with_progressbar() {
            log::error!("sync {} error: {:?}", repo_name, err);
        }
    }

    // for r in &config.repo {
    //     let mirror = MirrorBot::new(r).expect("");
    //     if let Err(err) = mirror.sync_with_progressbar() {
    //         log::error!("sync {} error: {:?}", r, err);
    //     }
    // }
}

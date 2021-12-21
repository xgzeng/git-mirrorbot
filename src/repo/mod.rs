mod progress;

use anyhow::{anyhow, Context, Result};
use serde_derive::Deserialize;
use std::path::{Path, PathBuf};

use progress::{FetchProgressHandler, ProgressIndicator};

#[derive(Deserialize, Debug, Clone)]
pub struct RepoConfig {
    url: String,
    path: String,
    #[serde(default)]
    mirror_urls: Vec<String>,
}

struct RepoMirror {
    config: RepoConfig,
}

#[derive(Default)]
pub struct RepoManager {
    // github_orgs: Vec<GithubOrg>,
    repo_list: Vec<RepoMirror>,
}

impl RepoMirror {
    fn local_path(&self) -> PathBuf {
        // PathBuf::from(self.config.path)
        let mut p = PathBuf::from("mirrors").join(&self.config.path);
        if p.extension().is_none() {
            p.set_extension("git");
        }
        p
    }

    pub fn sync(&self, progress_handler: Option<&mut dyn FetchProgressHandler>) -> Result<()> {
        if !self.local_path().exists() {
            log::info!(
                "{} not exists, init bare repository",
                self.local_path().display()
            );
            self.init()?;
        }
        self.fetch(progress_handler)?;
        Ok(())
    }

    fn init(&self) -> Result<()> {
        let repo_dir = self.local_path();
        log::info!("init bare repository {}", repo_dir.display());
        std::fs::create_dir_all(&repo_dir)?;
        let repo = git2::Repository::init_bare(&repo_dir)?;
        // create origin remote
        let _remote = repo.remote_with_fetch("origin", &self.config.url, "+refs/*:refs/*")?;
        let mut cfg = repo.config()?;
        let cfg_item = format!("remote.{}.mirror", "origin");
        cfg.set_bool(&cfg_item, true)?;
        Ok(())
    }

    fn fetch(&self, progress_handler: Option<&mut dyn FetchProgressHandler>) -> Result<()> {
        log::info!("fetch {}", self.config.url);
        let mut fetch_opts = git2::FetchOptions::new();
        if let Some(handler) = progress_handler {
            fetch_opts.remote_callbacks(handler.as_remote_callbacks());
        }

        log::info!("open bare repository {}", self.local_path().display());
        let repo = git2::Repository::open_bare(self.local_path())?;

        // check main url match with origin remote
        {
            let origin_remote = repo.find_remote("origin")?;
            if origin_remote.url().unwrap_or("") != self.config.url {
                return Err(anyhow!("origin remote url not match"));
            }
        }

        // determine mirror to use
        let mut remote = match self.config.mirror_urls.as_slice() {
            [url, ..] => {
                log::info!("user mirror {}", url);
                repo.remote_anonymous(url)?
            }
            _ => repo.find_remote("origin")?,
        };

        remote
            .connect(git2::Direction::Fetch)
            .context("connect to remote")?;

        log::info!("remote connected");
        // {
        //     log::info!("remote download");
        //     let fetch_opts = git2::FetchOptions::new();

        //     let callbacks: Option<git2::RemoteCallbacks<'_>> =
        //         progress_handler.map(|h| h.as_remote_callbacks());

        //     if let Some(cb) = callbacks {
        //         fetch_opts.remote_callbacks(cb);
        //     }

        //     let specs: Vec<&str> = vec![];
        //     remote.download(&specs, Some(&mut fetch_opts))?;
        // }
        // remote.update_tips(
        //     None, // callbacks1.as_mut(),
        //     true,
        //     git2::AutotagOption::All,
        //     Some("some message"),
        // )?;
        let empty_refs = Vec::<&str>::new();
        remote.fetch(&empty_refs, Some(&mut fetch_opts), None)?;
        Ok(())
    }
}

impl RepoManager {
    pub fn new() -> Self {
        RepoManager::default()
    }

    fn parse_github_repo_string(repo_str: &str) -> Result<RepoConfig> {
        let names: Vec<&str> = repo_str.split('/').collect();
        match &names[..] {
            [user, repo] => {
                let repo = RepoConfig {
                    url: format!("git://github.com/{}/{}.git", user, repo),
                    path: format!("github/{}/{}.git", user, repo),
                    mirror_urls: vec![], // vec![format!("https://hub.fastgit.org/{}/{}", user, repo)],
                };
                Ok(repo)
            }
            _ => {
                log::error!("invalid github repo name");
                Err(anyhow!("invalid github repo string"))
            }
        }
    }

    pub fn add_repo(&mut self, cfg: RepoConfig) -> Result<()> {
        if !Path::new(&cfg.path).is_relative() {
            return Err(anyhow!("path can only been relative"));
        }
        self.repo_list.push(RepoMirror { config: cfg });
        Ok(())
    }

    pub fn add_github_repo(&mut self, name: &str) -> Result<()> {
        log::info!("add github repository {}", name);
        if name.contains('/') {
            self.add_repo(Self::parse_github_repo_string(name)?)
        } else {
            // self.add_github_org(name)
            Err(anyhow!("mirror github user not supported yet"))
        }
    }

    pub fn update(&self) {
        for r in &self.repo_list {
            let mut progress_handler = ProgressIndicator::new();
            let sync_result = r.sync(Some(&mut progress_handler));
            drop(progress_handler);
            if let Err(err) = sync_result {
                log::error!("sync '{}' error: {:?}", r.config.url, err);
            }
        }
    }
}

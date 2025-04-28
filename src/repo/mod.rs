mod github;

mod progress;

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use progress::{FetchProgressHandler, ProgressIndicator};

#[derive(Debug, Clone)]
pub struct RepoConfig {
    url: String,  // main url
    path: String, // mirror url
    mirror_urls: Vec<String>,
}

// represent a single git repository
struct RepoMirror {
    url: String, // main url
    repo_dir: PathBuf,
    mirror_urls: Vec<String>,
}

impl RepoMirror {
    fn new(cfg: RepoConfig, base_storage_dir: &Path) -> Self {
        let mut repo_dir = base_storage_dir.join(cfg.path);
        if repo_dir.extension().is_none() {
            repo_dir.set_extension("git");
        }
        RepoMirror {
            url: cfg.url,
            repo_dir,
            mirror_urls: cfg.mirror_urls,
        }
    }

    pub fn sync(&self, fetch_options: Option<&mut git2::FetchOptions<'_>>) -> Result<()> {
        if !self.repo_dir.exists() {
            log::info!("{:?} not exists, init bare repository", self.repo_dir);
            self.init()?;
        }
        self.fetch(fetch_options)
    }

    fn init(&self) -> Result<()> {
        log::info!("init bare repository {:?}", self.repo_dir);
        std::fs::create_dir_all(&self.repo_dir)?;
        let repo = git2::Repository::init_bare(&self.repo_dir)?;
        // create origin remote
        let _remote = repo.remote_with_fetch("origin", &self.url, "+refs/*:refs/*")?;
        let mut cfg = repo.config()?;
        let cfg_item = format!("remote.{}.mirror", "origin");
        cfg.set_bool(&cfg_item, true)?;
        Ok(())
    }

    fn fetch(&self, fetch_options: Option<&mut git2::FetchOptions<'_>>) -> Result<()> {
        log::debug!("open bare repository {:?}", self.repo_dir);
        let repo = git2::Repository::open_bare(&self.repo_dir)?;
        // check main url match with origin remote
        {
            let r = repo.find_remote("origin")?;
            let origin_url = r.url().unwrap_or("");
            if origin_url != self.url {
                return Err(anyhow!(
                    "origin remote url {} not match {}",
                    origin_url,
                    self.url
                ));
            }
        }

        // determine mirror to use
        let mut remote = match self.mirror_urls.as_slice() {
            [url, ..] => {
                log::info!("user mirror {}", url);
                repo.remote_anonymous(url)?
            }
            _ => repo.find_remote("origin")?,
        };

        log::debug!("fetch {}", remote.url().unwrap());
        let refspecs = vec!["+refs/heads/*:refs/heads/*", "+refs/tags/*:refs/tags/*"];

        remote.fetch(&refspecs, fetch_options, None)?;
        Ok(())
    }
}

pub fn sync_with_progressbar(repo_name: &str, storage_dir: &Path) -> Result<()> {
    let repos = github::github_repos(repo_name)?;
    for repo_cfg in repos {
        log::info!("syncing {}", repo_cfg.url);
        let repo_mirror = RepoMirror::new(repo_cfg, storage_dir);
        let mut progress_handler = ProgressIndicator::new();
        let mut fetch_opts = git2::FetchOptions::default();
        fetch_opts.remote_callbacks(progress_handler.as_remote_callbacks());
        if let Err(err) = repo_mirror.sync(Some(&mut fetch_opts)) {
            log::error!("sync error: {}", err);
        }
    }

    Ok(())
}

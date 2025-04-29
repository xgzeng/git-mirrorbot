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
            self.init()?;
        }
        self.fetch(fetch_options)
    }

    fn init(&self) -> Result<()> {
        log::info!("init bare repository {:?}", self.repo_dir);
        std::fs::create_dir_all(&self.repo_dir)?;
        let repo = git2::Repository::init_bare(&self.repo_dir)?;
        // create origin remote
        repo.remote_with_fetch("origin", &self.url, "+refs/*:refs/*")?;
        repo.config()?.set_bool("remote.origin.mirror", true)?;
        Ok(())
    }

    fn fetch(&self, fetch_options: Option<&mut git2::FetchOptions<'_>>) -> Result<()> {
        log::debug!("open bare repository {:?}", self.repo_dir);
        let repo = git2::Repository::open_bare(&self.repo_dir)?;
        let mut remote = repo.find_remote("origin")?;
        log::debug!("fetch {}", remote.url().unwrap());
        let refspecs = vec!["+refs/heads/*:refs/heads/*", "+refs/tags/*:refs/tags/*"];
        remote.fetch(&refspecs, fetch_options, None)?;
        Ok(())
    }
}

pub fn sync_with_progressbar(repo_name: &str, storage_dir: &Path) -> Result<()> {
    let repos = github::github_repos(repo_name)?;
    for repo_cfg in repos {
        log::info!("sync {}", repo_cfg.url);
        let repo_mirror = RepoMirror::new(repo_cfg, storage_dir);
        let mut progress_handler = ProgressIndicator::new();
        let mut fetch_opts = git2::FetchOptions::default();
        fetch_opts.remote_callbacks(progress_handler.as_remote_callbacks());
        if let Err(err) = repo_mirror.sync(Some(&mut fetch_opts)) {
            log::error!("sync error: {}", err);
        } else {
            log::info!("sync done");
        }
    }

    Ok(())
}

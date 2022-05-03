mod github;

mod progress;

use anyhow::{anyhow, Result};
use std::path::PathBuf;

use progress::{FetchProgressHandler, ProgressIndicator};

pub struct RepoConfig {
    url: String,  // main url
    path: String, // mirror url
    mirror_urls: Vec<String>,
}

// represent a single git repository
struct RepoMirror {
    url: String,  // main url
    path: String, // mirror url
    mirror_urls: Vec<String>,
}

impl RepoMirror {
    fn new(cfg: RepoConfig) -> Self {
        RepoMirror {
            url: cfg.url,
            path: cfg.path,
            mirror_urls: cfg.mirror_urls,
        }
    }

    fn local_path(&self) -> PathBuf {
        let mut p = PathBuf::from("mirrors").join(&self.path);
        if p.extension().is_none() {
            p.set_extension("git");
        }
        p
    }

    pub fn sync(&self, fetch_options: Option<&mut git2::FetchOptions<'_>>) -> Result<()> {
        if !self.local_path().exists() {
            log::info!(
                "{} not exists, init bare repository",
                self.local_path().display()
            );
            self.init()?;
        }
        self.fetch(fetch_options)
    }

    fn init(&self) -> Result<()> {
        let repo_dir = self.local_path();
        log::info!("init bare repository {}", repo_dir.display());
        std::fs::create_dir_all(&repo_dir)?;
        let repo = git2::Repository::init_bare(&repo_dir)?;
        // create origin remote
        let _remote = repo.remote_with_fetch("origin", &self.url, "+refs/*:refs/*")?;
        let mut cfg = repo.config()?;
        let cfg_item = format!("remote.{}.mirror", "origin");
        cfg.set_bool(&cfg_item, true)?;
        Ok(())
    }

    fn fetch(&self, fetch_options: Option<&mut git2::FetchOptions<'_>>) -> Result<()> {
        log::info!("fetch {}", self.url);

        log::info!("open bare repository {}", self.local_path().display());
        let repo = git2::Repository::open_bare(self.local_path())?;

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

        let empty_refs = Vec::<&str>::new();
        remote.fetch(&empty_refs, fetch_options, None)?;
        Ok(())
    }
}

pub trait RepoProvider {
    // get repo list
    fn repos(&self) -> Result<Box<dyn Iterator<Item = RepoConfig>>>;
}

pub struct MirrorBot {
    repo_provider: Box<dyn RepoProvider>,
}

impl MirrorBot {
    pub fn from_simple_name(name: &str) -> Result<Self> {
        let repo_provider: Box<dyn RepoProvider>;
        if name.contains('/') {
            repo_provider = Box::new(github::GithubSingleRepo::new(name)?);
        } else {
            repo_provider = Box::new(github::GithubUserRepos::new(name));
        }

        Ok(MirrorBot { repo_provider })
    }

    pub fn sync_with_progressbar(&self) -> Result<()> {
        let mut progress_handler = ProgressIndicator::new();
        //
        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(progress_handler.as_remote_callbacks());

        let mut proxy_options = git2::ProxyOptions::new();
        proxy_options.url("http://127.0.0.1:8080");

        fetch_opts.proxy_options(proxy_options);

        // self.config.sync(Some(&mut progress_handler))
        for repo_cfg in self.repo_provider.repos()? {
            let repo_mirror = RepoMirror::new(repo_cfg);
            if let Err(err) = repo_mirror.sync(Some(&mut fetch_opts)) {
                log::error!("sync error: {}", err);
            }
        }

        Ok(())
    }
}

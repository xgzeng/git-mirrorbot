mod progress;

use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

use progress::{FetchProgressHandler, ProgressIndicator};

struct RepoMirror {
    url: String,
    path: PathBuf,
}

#[derive(Default)]
pub struct RepoManager {
    // github_orgs: Vec<GithubOrg>,
    repo_list: Vec<RepoMirror>,
}

impl RepoMirror {
    pub fn sync(&self, progress_cb: Option<git2::RemoteCallbacks>) -> Result<()> {
        if !self.path.exists() {
            log::info!("{} not exists, sync by clone", self.path.display());
            self.clone(progress_cb)?;
        } else {
            log::info!("{} already exists, sync by fetch", self.path.display());
            self.fetch(progress_cb)?;
        }
        Ok(())
    }

    fn clone(&self, progress_cb: Option<git2::RemoteCallbacks>) -> Result<()> {
        // Proxy Options
        // let mut proxy_opts = git2::ProxyOptions::new();
        // build and clone
        let mut fetch_options = git2::FetchOptions::new();
        match progress_cb {
            Some(cb) => {
                fetch_options.remote_callbacks(cb);
                ()
            }
            _ => (),
        }
        // fetch_options.proxy_options(proxy_opts);

        let mut builder = git2::build::RepoBuilder::new();
        builder.bare(true);
        builder.fetch_options(fetch_options);
        builder.remote_create(|repo, name, url| {
            // Create remote with a mirror refspec
            let remote = repo.remote_with_fetch(name, url, "+refs/*:refs/*")?;
            // Set the mirror setting to true on this remote
            let mut cfg = repo.config()?;
            let cfg_item = format!("remote.{}.mirror", name);
            cfg.set_bool(&cfg_item, true)?;
            Ok(remote)
        });

        builder.clone(&self.url, &self.path)?;
        Ok(())
    }

    fn fetch(&self, progress_cb: Option<git2::RemoteCallbacks>) -> Result<()> {
        let repo = git2::Repository::open_bare(&self.path)?;
        let mut remote = repo.find_remote("origin")?;
        // let mut refspecs = remote.fetch_refspecs()?;

        log::info!("connect {}", remote.url().expect("remote no url"));
        remote
            .connect(git2::Direction::Fetch)
            .context("connect to remote")?;

        let mut fetch_opts = git2::FetchOptions::new();
        match progress_cb {
            Some(cb) => {
                fetch_opts.remote_callbacks(cb);
                ()
            }
            _ => (),
        }

        log::info!("remote download");
        let specs: Vec<&str> = vec![];
        remote.download(&specs, Some(&mut fetch_opts))?;

        remote.update_tips(
            None, // progress_cb.as_mut(),
            true,
            git2::AutotagOption::All,
            Some("some message"),
        )?;
        // remote.fetch(, Some(&mut fetch_options), None)?;
        Ok(())
    }
}

impl RepoManager {
    pub fn new() -> Self {
        RepoManager::default()
    }

    // pub fn add_github_org(&mut self, org_name: &String) {
    //     log::error!("github organization not supported yet");
    //     log::info!("add github organization {}", org_name);
    //     self.github_orgs.push(GithubOrg {
    //         name: org_name.clone(),
    //         repo_list: vec![],
    //     });
    // }
    fn parse_github_repo_string(repo_str: &String) -> Result<RepoMirror> {
        let names: Vec<&str> = repo_str.split('/').collect();
        match &names[..] {
            [org_name, repo_name] => {
                let repo = RepoMirror {
                    url: format!("https://github.com/{}/{}.git", org_name, repo_name),
                    path: PathBuf::from(format!("mirrors/{}/{}.git", org_name, repo_name)),
                };
                Ok(repo)
            }
            _ => {
                log::error!("invalid github repo name");
                Err(anyhow!("invalid github repo string"))
            }
        }
    }
    pub fn add_github_repo(&mut self, name: &String) -> Result<()> {
        log::info!("add github repository {}", name);
        if name.contains("/") {
            let repo = Self::parse_github_repo_string(name)?;
            self.repo_list.push(repo);
            Ok(())
        } else {
            // self.add_github_org(name)
            Err(anyhow!("github organization not supported yet"))
        }
    }

    pub fn update(&self) {
        for r in &self.repo_list {
            let mut progress_handler = ProgressIndicator::new();
            match r.sync(Some(progress_handler.as_remote_callbacks())) {
                // to_remote_callbacks(&mut progress_handler)
                Err(err) => log::error!("update '{}' error: {:?}", r.url, err),
                _ => (),
            }
        }
    }
}

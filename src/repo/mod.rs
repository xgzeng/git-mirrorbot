use anyhow::{anyhow, Result};
use std::path::PathBuf;

type ProgressCallback = Box<dyn Fn(usize, usize)>;

struct RepoSetting {
    url: String,
    path: PathBuf,
}

// pub struct GithubOrg {
//     name: String,
//     repo_list: Vec<RepoSetting>,
// }

#[derive(Default)]
pub struct RepoManager {
    // github_orgs: Vec<GithubOrg>,
    repo_list: Vec<RepoSetting>,
}

fn create_mirror_remote<'a>(
    repo: &'a git2::Repository,
    name: &str,
    url: &str,
) -> std::result::Result<git2::Remote<'a>, git2::Error> {
    repo.remote_with_fetch(name, url, "+refs/*:refs/*")
    // return Ok(origin_remote);
    // log::error!("create_mirror_remote: {} {}", name, msg);
    // Err(git2::Error::from_str("not implemented"))
}

impl RepoSetting {
    pub fn update(&self, progress_cb: Option<ProgressCallback>) -> Result<()> {
        if !self.path.exists() {
            log::info!("{} not exists, do git-clone", self.path.display());
            self.clone(progress_cb)?;
        } else {
            log::info!("{} already exists, do git-fetch", self.path.display());
            self.fetch(progress_cb)?;
        }
        Ok(())
    }

    fn progress_log_callback(f: Option<ProgressCallback>) -> git2::RemoteCallbacks<'static> {
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.sideband_progress(|msg: &[u8]| {
            log::info!("sideband_progress: {}", String::from_utf8_lossy(msg));
            true
        });

        callbacks.transfer_progress(move |p: git2::Progress| {
            match &f {
                Some(cb) => cb(p.total_objects(), p.received_objects()),
                _ => (),
            }
            // log::info!(
            //     "objects: total {}, received {},",
            //     p.total_objects(),
            //     p.received_objects()
            // );
            true
        });

        callbacks.update_tips(|a, b, c| {
            log::info!("update_tips: {} {} {}", a, b, c);
            true
        });

        callbacks.pack_progress(|stage, m, n| {
            log::info!("pack_progress: {:?} {} {}", stage, m, n);
        });
        callbacks
    }

    fn clone(&self, progress_cb: Option<ProgressCallback>) -> Result<()> {
        // Proxy Options
        // let mut proxy_opts = git2::ProxyOptions::new();

        // build and clone
        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(Self::progress_log_callback(progress_cb));
        // fetch_options.proxy_options(proxy_opts);

        let mut builder = git2::build::RepoBuilder::new();
        builder.bare(true);
        builder.fetch_options(fetch_options);
        builder.remote_create(create_mirror_remote);

        builder.clone(&self.url, &self.path)?;
        Ok(())
    }

    fn fetch(&self, progress_cb: Option<ProgressCallback>) -> Result<()> {
        let repo = git2::Repository::open_bare(&self.path)?;
        let mut remote = repo.find_remote("origin")?;
        // let mut refspecs = remote.fetch_refspecs()?;

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(Self::progress_log_callback(progress_cb));
        remote.fetch(&["+refs/*:refs/*"], Some(&mut fetch_options), None)?;
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
    fn parse_github_repo_string(repo_str: &String) -> Result<RepoSetting> {
        let names: Vec<&str> = repo_str.split('/').collect();
        match &names[..] {
            [org_name, repo_name] => {
                let repo = RepoSetting {
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
            let indicator = indicatif::ProgressBar::new(100);
            let pcb = move |total: usize, received: usize| {
                let total_u64 = total.try_into().unwrap();
                if total_u64 != indicator.length() {
                    indicator.set_length(total_u64);
                }

                let received_u64 = received.try_into().unwrap();
                indicator.set_position(received_u64);
            };

            match r.update(Some(Box::new(pcb))) {
                Err(err) => log::error!("update '{}' error: {}", r.url, err),
                _ => (),
            }
        }
    }
}

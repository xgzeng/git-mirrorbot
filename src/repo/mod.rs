use anyhow::{anyhow, Result};

struct RepoSetting {
    url: String,
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

fn parse_github_repo_string(repo_str: &String) -> Result<RepoSetting> {
    let names: Vec<&str> = repo_str.split('/').collect();
    match &names[..] {
        [org_name, repo_name] => {
            let repo = RepoSetting {
                url: format!("https://github.com/{}/{}.git", org_name, repo_name),
            };
            Ok(repo)
        }
        _ => {
            log::error!("invalid github repo name");
            Err(anyhow!("invalid github repo string"))
        }
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

    pub fn add_github_repo(&mut self, name: &String) -> Result<()> {
        log::info!("add github repository {}", name);
        if name.contains("/") {
            let repo = parse_github_repo_string(name)?;
            self.repo_list.push(repo);
            Ok(())
        } else {
            // self.add_github_org(name)
            Err(anyhow!("github organization not supported yet"))
        }
    }
}

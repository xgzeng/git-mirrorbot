use crate::repo::{RepoConfig, RepoProvider};
use anyhow::{anyhow, Result};
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
struct RepoInfo {
    name: String,
    fork: bool,
}

#[derive(Deserialize, Debug)]
struct UserInfo {
    public_repos: i32,
}

fn github_get(url: &str) -> reqwest::Result<reqwest::blocking::Response> {
    reqwest::blocking::Client::builder()
        .user_agent("curl/7.77.0")
        .build()? // ClientBuilder
        .get(url) // RequestBuilder
        .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
        .send()
}

fn get_github_user_count(user: &str) -> Result<i32> {
    let url = format!("https://api.github.com/users/{}", user);
    log::info!("GET {}", url);
    let user_info = github_get(&url)?.json::<UserInfo>()?;
    Ok(user_info.public_repos)
}

// fn list_github_user_repos_page(user: &str, page_size: i32, page: i32) -> Result<Vec<String>> {
//     let url = format!(
//         "https://api.github.com/users/{}/repos?per_page={}&page={}",
//         user, page_size, page
//     );
//     log::info!("GET {}", url);
//     let names = github_get(&url)?
//         .json::<Vec<RepoInfo>>()?
//         .into_iter()
//         .filter(|info| info.fork)
//         .map(|info| info.name)
//         .collect::<Vec<String>>();
//     log::info!("Got repos: {:?}", names);
//     Ok(names)
// }

pub fn list_github_user_repos(user: &str) -> Result<Vec<String>> {
    let repo_count = get_github_user_count(user)?;
    if repo_count > 100 {
        return Err(anyhow!("too many repos for user"));
    }

    const GET_PAGE_SIZE: i32 = 50;
    let mut fork_repo_count = 0;
    let mut repo_names = vec![];

    // get user repos page by page
    for page in 1..3 {
        let url = format!(
            "https://api.github.com/users/{}/repos?per_page={}&page={}",
            user, GET_PAGE_SIZE, page
        );
        log::info!("GET {}", url);

        let repo_infos = github_get(&url)?.json::<Vec<RepoInfo>>()?;
        let last_page = (repo_infos.len() as i32) < GET_PAGE_SIZE;

        let mut names = repo_infos
            .into_iter()
            .filter(|info| {
                if info.fork {
                    fork_repo_count += 1;
                };
                !info.fork
            })
            .map(|info| info.name)
            .collect::<Vec<String>>();

        log::info!("Got repos: {:?}", names);

        repo_names.append(&mut names);
        if last_page {
            break;
        }
    }

    log::info!(
        "user has {} repos, skip {} forked repos, sync {} repos",
        repo_count,
        fork_repo_count,
        repo_names.len()
    );

    Ok(repo_names)
}

pub struct GithubUserRepos {
    user: String,
}

impl GithubUserRepos {
    pub fn new(name: &str) -> Self {
        GithubUserRepos {
            user: String::from(name),
        }
    }
}

impl RepoProvider for GithubUserRepos {
    fn repos(&self) -> Result<Box<dyn Iterator<Item = RepoConfig>>> {
        let repo_names = list_github_user_repos(&self.user)?;
        let user = self.user.clone();
        let iters = repo_names.into_iter().map(move |name| RepoConfig {
            url: format!("https://github.com/{}/{}.git", user, name),
            path: format!("github/{}/{}.git", user, name),
            mirror_urls: vec![],
        });

        Ok(Box::new(iters))
    }
}

// repositories represented by an single name
pub struct GithubSingleRepo {
    user: String,
    repo: String,
}

impl RepoProvider for GithubSingleRepo {
    fn repos(&self) -> Result<Box<dyn Iterator<Item = RepoConfig>>> {
        let url = format!("https://github.com/{}/{}.git", self.user, self.repo);
        let path = format!("github/{}/{}.git", self.user, self.repo);
        let repo = RepoConfig {
            url,
            path,
            mirror_urls: vec![],
        };

        // mirror_urls: vec![], // vec![format!("https://hub.fastgit.org/{}/{}", user, repo)],
        Ok(Box::new(std::iter::once(repo)))
    }
}

impl GithubSingleRepo {
    pub fn new(name: &str) -> Result<Self> {
        let names: Vec<&str> = name.split('/').collect();
        match names[..] {
            [user, repo] => Ok(GithubSingleRepo {
                user: String::from(user),
                repo: String::from(repo),
            }),
            _ => {
                log::error!("invalid short repo name");
                Err(anyhow!("invalid github repo string"))
            }
        }
    }
}

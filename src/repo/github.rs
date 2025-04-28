use crate::repo::RepoConfig;
use anyhow::{anyhow, Result};
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
struct RepoInfo {
    name: String,
    fork: bool,
}

#[derive(Deserialize, Debug)]
struct UserInfo {
    public_repos: usize,
}

fn github_get(url: &str) -> reqwest::Result<reqwest::blocking::Response> {
    reqwest::blocking::Client::builder()
        .user_agent("curl/7.77.0")
        .build()? // ClientBuilder
        .get(url) // RequestBuilder
        .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
        .send()
}

fn get_github_user_public_repos_count(user: &str) -> Result<usize> {
    let url = format!("https://api.github.com/users/{}", user);
    log::debug!("GET {}", url);
    let user_info = github_get(&url)?.json::<UserInfo>()?;
    Ok(user_info.public_repos)
}

pub fn list_github_user_repos(user: &str) -> Result<Vec<String>> {
    let repo_count = get_github_user_public_repos_count(user)?;

    const N_PERPAGE: usize = 50;
    let mut fork_repo_count = 0;
    let mut repo_names = vec![];

    for page in 1..repo_count / N_PERPAGE + 2 {
        let url = format!(
            "https://api.github.com/users/{}/repos?per_page={}&page={}",
            user, N_PERPAGE, page
        );
        log::debug!("GET {}", url);

        let repo_infos = github_get(&url)?.json::<Vec<RepoInfo>>()?;

        let mut names = repo_infos
            .iter()
            .filter(|info| {
                if info.fork {
                    fork_repo_count += 1;
                };
                !info.fork
            })
            .map(|info| info.name.clone())
            .collect::<Vec<String>>();

        log::debug!("Got repos: {:?}", names);
        repo_names.append(&mut names);

        if repo_infos.len() < N_PERPAGE {
            // last page
            break;
        }
    }

    log::info!(
        "{} has {} repos, skip {} forked repos, sync {} repos",
        user,
        repo_count,
        fork_repo_count,
        repo_names.len()
    );

    Ok(repo_names)
}

fn repos_of_user(user: &str) -> Result<Vec<RepoConfig>> {
    let repo_names = list_github_user_repos(user)?;
    let repos = repo_names
        .into_iter()
        .map(move |name| RepoConfig {
            url: format!("https://github.com/{}/{}.git", user, name),
            path: format!("github/{}/{}.git", user, name),
            mirror_urls: vec![],
        })
        .collect();
    Ok(repos)
}

pub fn github_repos(id: &str) -> Result<Vec<RepoConfig>> {
    let names: Vec<&str> = id.split('/').collect();
    match names[..] {
        [user, repo] => {
            let url = format!("https://github.com/{}/{}.git", user, repo);
            let path = format!("github/{}/{}.git", user, repo);
            let repo = RepoConfig {
                url,
                path,
                mirror_urls: vec![],
            };
            Ok(vec![repo])
        }
        [user] => repos_of_user(user),
        _ => Err(anyhow!("invalid repo name {}", id)),
    }
}

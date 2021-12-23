use anyhow::{anyhow, Context, Result};
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
struct RepoInfo {
    name: String,
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

fn list_github_user_repos_page(user: &str, page_size: i32, page: i32) -> Result<Vec<String>> {
    let url = format!(
        "https://api.github.com/users/{}/repos?per_page={}&page={}",
        user, page_size, page
    );
    log::info!("GET {}", url);
    let names = github_get(&url)?
        .json::<Vec<RepoInfo>>()?
        .into_iter()
        .map(|info| info.name)
        .collect::<Vec<String>>();
    log::info!("Got repos: {:?}", names);
    Ok(names)
}

pub fn list_github_user_repos(user: &str) -> Result<Vec<String>> {
    let repo_count = get_github_user_count(user)?;
    if repo_count > 100 {
        return Err(anyhow!("too many repos for user"));
    }

    const GET_PAGE_SIZE: i32 = 50;
    let mut repo_names = vec![];
    for n in 1..3 {
        let mut page_names = list_github_user_repos_page(user, GET_PAGE_SIZE, n)?;
        repo_names.append(&mut page_names);
        if page_names.len() < GET_PAGE_SIZE as usize {
            break;
        }
    }

    if repo_names.len() != repo_count as usize {
        log::warn!(
            "user has {} repos, only fetched {}",
            repo_count,
            repo_names.len()
        );
    }

    Ok(repo_names)
}

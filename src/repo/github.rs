use anyhow::Result;

async fn list_github_user_repos_async(user: &str) -> Result<Vec<String>> {
    let octocrab = octocrab::Octocrab::builder().build()?;

    let mut page = octocrab.users(user).repos().per_page(50).send().await?;

    let mut repo_names = vec![];
    let mut fork_count = 0;

    loop {
        for repo in &page {
            if repo.fork.unwrap_or(false) {
                fork_count += 1;
            } else {
                repo_names.push(repo.name.clone());
            }
        }

        page = match octocrab.get_page(&page.next).await? {
            Some(next) => next,
            None => break,
        };
    }

    log::info!(
        "{}: syncing {} repos, skipping {} forked repos",
        user,
        repo_names.len(),
        fork_count
    );

    Ok(repo_names)
}

pub fn list_user_repos(user: &str) -> Result<Vec<String>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(list_github_user_repos_async(user))
}

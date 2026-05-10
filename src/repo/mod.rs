mod github;

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RepoConfig {
    url: String,  // main url
    path: String, // mirror url
    mirror_urls: Vec<String>,
    sync_interval: Duration,
}

const DEFAULT_SYNC_INTERVAL: Duration = Duration::from_secs(60 * 60 * 24); // 1 day

impl RepoConfig {
    pub fn new(url: String, path: String) -> Self {
        Self {
            url,
            path,
            mirror_urls: vec![],
            sync_interval: DEFAULT_SYNC_INTERVAL,
        }
    }
}

#[derive(serde_derive::Serialize, serde_derive::Deserialize, Default)]
struct MirrorState {
    /// RFC 3339 timestamp of the last successful sync, e.g. "2026-05-10T12:34:56Z".
    last_sync_time: Option<String>,
}

/// Format a `SystemTime` as an RFC 3339 / ISO 8601 UTC string.
fn format_rfc3339(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, mo, d, h, mi, s) = epoch_secs_to_datetime(secs);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
}

/// Parse an RFC 3339 UTC string back to seconds since epoch.
fn parse_rfc3339(s: &str) -> Option<u64> {
    // Expected format: YYYY-MM-DDThh:mm:ssZ
    let b = s.as_bytes();
    if b.len() < 20 {
        return None;
    }
    let year: u64 = std::str::from_utf8(&b[0..4]).ok()?.parse().ok()?;
    let month: u64 = std::str::from_utf8(&b[5..7]).ok()?.parse().ok()?;
    let day: u64 = std::str::from_utf8(&b[8..10]).ok()?.parse().ok()?;
    let hour: u64 = std::str::from_utf8(&b[11..13]).ok()?.parse().ok()?;
    let min: u64 = std::str::from_utf8(&b[14..16]).ok()?.parse().ok()?;
    let sec: u64 = std::str::from_utf8(&b[17..19]).ok()?.parse().ok()?;
    // Days from epoch to start of year
    let days_to_year: u64 = (1970..year)
        .map(|y| if is_leap(y) { 366 } else { 365 })
        .sum();
    let month_days: [u64; 12] = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let days_to_month: u64 = month_days[..((month as usize).saturating_sub(1))]
        .iter()
        .sum();
    let days = days_to_year + days_to_month + day.saturating_sub(1);
    Some(days * 86400 + hour * 3600 + min * 60 + sec)
}

fn is_leap(y: u64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn epoch_secs_to_datetime(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let mins = secs / 60;
    let mi = mins % 60;
    let hours = mins / 60;
    let h = hours % 24;
    let mut days = hours / 24;
    let mut year = 1970u64;
    loop {
        let dy = if is_leap(year) { 366 } else { 365 };
        if days < dy {
            break;
        }
        days -= dy;
        year += 1;
    }
    let month_days: [u64; 12] = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1, h, mi, s)
}

impl MirrorState {
    fn load(repo_dir: &Path) -> Self {
        let path = repo_dir.join("mirror_state.json");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self, repo_dir: &Path) -> Result<()> {
        let path = repo_dir.join("mirror_state.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

// represent a single git repository
struct RepoMirror {
    url: String, // main url
    repo_dir: PathBuf,
    mirror_urls: Vec<String>,
    sync_interval: Duration,
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
            sync_interval: cfg.sync_interval,
        }
    }

    pub fn need_sync(&self) -> bool {
        let state = MirrorState::load(&self.repo_dir);
        if let Some(ref ts) = state.last_sync_time {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let last_sync = parse_rfc3339(ts).unwrap_or(0);
            let elapsed = Duration::from_secs(now.saturating_sub(last_sync));
            return elapsed >= self.sync_interval;
        }
        true
    }

    pub fn sync(&self) -> Result<()> {
        if !self.repo_dir.exists() {
            self.init()?;
        }
        // self.fetch(fetch_options)
        self.fetch_with_subprocess()?;
        let ts = format_rfc3339(std::time::SystemTime::now());
        MirrorState {
            last_sync_time: Some(ts),
        }
        .save(&self.repo_dir)?;
        Ok(())
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
        let repo = git2::Repository::open_bare(&self.repo_dir)?;
        let mut remote = repo.find_remote("origin")?;
        log::info!("fetch {} @ {:?}", self.url, self.repo_dir);
        let refspecs = vec!["+refs/heads/*:refs/heads/*", "+refs/tags/*:refs/tags/*"];
        remote.fetch(&refspecs, fetch_options, None)?;
        Ok(())
    }

    // fetch with git subprocess, which is more stable
    fn fetch_with_subprocess(&self) -> Result<()> {
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.repo_dir.join("git_command.log"))?;
        let log_file_stderr = log_file.try_clone()?;
        let status = std::process::Command::new("git")
            .args(["fetch", "--all", "--prune"])
            .current_dir(&self.repo_dir)
            .stdout(log_file)
            .stderr(log_file_stderr)
            .status()?;
        if !status.success() {
            log::error!(
                "git fetch failed, see log file {:?}",
                self.repo_dir.join("git_command.log")
            );
            return Err(anyhow::anyhow!("git fetch failed with {}", status));
        }
        Ok(())
    }
}

pub fn sync(repo_name: &str, storage_dir: &Path) -> Result<()> {
    let repos = github::github_repos(repo_name)?;
    for repo_cfg in repos {
        let repo_mirror = RepoMirror::new(repo_cfg, storage_dir);
        if !repo_mirror.need_sync() {
            log::info!("skip sync {}", repo_mirror.url);
            continue;
        }
        log::info!("start sync {}", repo_mirror.url);
        if let Err(err) = repo_mirror.sync() {
            log::error!("sync {} error: {}", err, repo_mirror.url);
        } else {
            log::info!("sync success");
        }
    }

    Ok(())
}

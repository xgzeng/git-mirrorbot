use anyhow::{anyhow, Result};
use serde_derive::Deserialize;
use std::path::{Path, PathBuf};

fn default_storage_dir() -> PathBuf {
    PathBuf::from("./mirrors")
}

#[derive(Deserialize, Debug)]
pub struct AppConfig {
    pub mirrors: Vec<String>,

    #[serde(default = "default_storage_dir")]
    pub storage_dir: PathBuf,
}

pub fn from_file(file_path: &Path) -> Result<AppConfig> {
    if !file_path.exists() {
        return Err(anyhow!("Config file not found: {}", file_path.display()));
    }
    let file = std::fs::File::open(file_path)?;
    let reader = std::io::BufReader::new(file);
    let config: AppConfig = serde_yml::from_reader(reader)?;
    Ok(config)
}

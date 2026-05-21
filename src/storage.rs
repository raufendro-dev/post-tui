use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::models::{Collection, HistoryItem, RequestItem};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub response_pretty: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            response_pretty: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Storage {
    data_dir: PathBuf,
}

impl Storage {
    pub fn new() -> Result<Self> {
        let project_dirs = ProjectDirs::from("dev", "post-tui", "post-tui")
            .context("could not determine an application data directory")?;
        let data_dir = project_dirs.data_local_dir().to_path_buf();
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("failed to create {}", data_dir.display()))?;
        Ok(Self { data_dir })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn load_collections(&self) -> Vec<Collection> {
        self.load_json("collections.json").unwrap_or_default()
    }

    pub fn save_collections(&self, collections: &[Collection]) -> Result<()> {
        self.save_json("collections.json", collections)
    }

    pub fn load_history(&self) -> Vec<HistoryItem> {
        self.load_json("history.json").unwrap_or_default()
    }

    pub fn save_history(&self, history: &[HistoryItem]) -> Result<()> {
        self.save_json("history.json", history)
    }

    pub fn clear_history(&self) -> Result<()> {
        self.save_history(&[])
    }

    pub fn delete_html_exports(&self) -> Result<usize> {
        let mut deleted = 0;
        for entry in fs::read_dir(&self.data_dir)
            .with_context(|| format!("failed to read {}", self.data_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let is_html = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.eq_ignore_ascii_case("html"))
                .unwrap_or(false);

            if path.is_file() && is_html {
                fs::remove_file(&path)
                    .with_context(|| format!("failed to delete {}", path.display()))?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    pub fn load_saved_requests(&self) -> Vec<RequestItem> {
        self.load_json("requests.json").unwrap_or_default()
    }

    pub fn save_saved_requests(&self, requests: &[RequestItem]) -> Result<()> {
        self.save_json("requests.json", requests)
    }

    pub fn load_config(&self) -> AppConfig {
        let path = self.data_dir.join("config.toml");
        let Ok(contents) = fs::read_to_string(path) else {
            return AppConfig::default();
        };
        toml::from_str(&contents).unwrap_or_default()
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<()> {
        let path = self.data_dir.join("config.toml");
        let contents = toml::to_string_pretty(config)?;
        fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))
    }

    fn load_json<T: DeserializeOwned>(&self, file_name: &str) -> Result<T> {
        let path = self.data_dir.join(file_name);
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    fn save_json<T: Serialize + ?Sized>(&self, file_name: &str, value: &T) -> Result<()> {
        let path = self.data_dir.join(file_name);
        let contents = serde_json::to_string_pretty(value)?;
        fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))
    }
}

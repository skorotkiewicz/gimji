use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::storage::atomic::atomic_write;

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct RecentWorkspaces {
    pub(super) paths: Vec<PathBuf>,
}

impl RecentWorkspaces {
    pub(super) fn add(&mut self, path: PathBuf) {
        self.paths.retain(|recent| recent != &path);
        self.paths.insert(0, path);
        self.paths.truncate(8);
    }

    pub(super) fn remove(&mut self, path: &Path) -> bool {
        let original_len = self.paths.len();
        self.paths.retain(|recent| recent != path);
        self.paths.len() != original_len
    }

    pub(super) fn load(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub(super) fn save(&self, path: &Path) {
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let _ = atomic_write(path, &bytes);
        }
    }
}

pub(super) fn recent_workspaces_path() -> Option<PathBuf> {
    ProjectDirs::from("dev", "mod", "Gimji")
        .map(|project_dirs| project_dirs.config_dir().join("recent_workspaces.json"))
}

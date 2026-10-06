//! Config persistence, replacing the Swift `UserDefaults["solverlab.simconfig.v1"]`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::model::SimConfig;

const FILE_NAME: &str = "simconfig.v1.json";

#[derive(Clone, Debug, Default)]
pub struct ConfigStore {
    /// `None` disables persistence.
    path: Option<PathBuf>,
}

impl ConfigStore {
    pub fn disabled() -> Self {
        ConfigStore { path: None }
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        ConfigStore {
            path: Some(path.into()),
        }
    }

    /// Platform config dir, e.g. `~/.config/solverlab/simconfig.v1.json` on Linux.
    pub fn default_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "jfg", "solverlab").map(|dirs| dirs.config_dir().join(FILE_NAME))
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Missing or unreadable files fall back to the defaults.
    pub fn load(&self) -> SimConfig {
        self.path
            .as_ref()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, config: &SimConfig) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let json = serde_json::to_string_pretty(config).map_err(io::Error::other)?;
        write_atomic(path, json.as_bytes())
    }
}

/// Writes to a sibling temp file and renames it over `path`.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GameType;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::at(dir.path().join("nested/config.json"));
        let config = SimConfig {
            game_type: GameType::TriPeaks,
            simulations: 25,
            parallel_games: 3,
            auto_parallel: true,
            ..SimConfig::default()
        };
        store.save(&config).unwrap();
        assert_eq!(store.load(), config);
    }

    #[test]
    fn missing_or_broken_file_loads_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let store = ConfigStore::at(&path);
        assert_eq!(store.load(), SimConfig::default());

        fs::write(&path, "{not json").unwrap();
        assert_eq!(store.load(), SimConfig::default());
    }

    #[test]
    fn legacy_partial_json_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            r#"{"gameType":"Pyramid","simulations":7,"parallelGames":2,"autoParallel":false}"#,
        )
        .unwrap();
        let config = ConfigStore::at(&path).load();
        assert_eq!(config.game_type, GameType::Pyramid);
        assert_eq!(config.simulations, 7);
        assert_eq!(config.max_depth, 150);
        assert_eq!(config.max_undos, -1);
    }

    #[test]
    fn disabled_store_never_writes() {
        let store = ConfigStore::disabled();
        assert!(store.path().is_none());
        store.save(&SimConfig::default()).unwrap();
        assert_eq!(store.load(), SimConfig::default());
    }
}

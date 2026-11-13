//! The saved login: the same file the `probnode` command line (Python) reads and writes.
//!
//! The file is JSON, `{"api_key": "...", "base_url": "..."}`, at `$PROBNODE_CONFIG` or else
//! `~/.config/probnode.json` (on Windows, `%USERPROFILE%\.config\probnode.json`). A key saved by
//! `probnode login` is found here, and a key saved here is found by `probnode`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// What `probnode login` saves.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// The API key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// The API base URL the key belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Anything else in the file, kept as it was.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Where the config lives: `$PROBNODE_CONFIG`, else `~/.config/probnode.json`. `None` only when
/// neither that variable nor a home directory is set.
pub fn path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("PROBNODE_CONFIG").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    home().map(|h| h.join(".config").join("probnode.json"))
}

fn home() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .or_else(|| std::env::var_os("HOME"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

impl Config {
    /// The saved config, or an empty one when there is no file.
    pub fn load() -> Result<Config> {
        match path() {
            Some(p) => Config::load_from(&p),
            None => Ok(Config::default()),
        }
    }

    /// The config at `path`, or an empty one when the file does not exist.
    pub fn load_from(path: &Path) -> Result<Config> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(Error::Io(e)),
        }
    }

    /// Writes the config where [`path`] says, readable by the owner only on Unix.
    pub fn save(&self) -> Result<PathBuf> {
        let p = path().ok_or_else(|| Error::Invalid("no home directory to save into".into()))?;
        self.save_to(&p)?;
        Ok(p)
    }

    /// Writes the config to `path`, creating its directory.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(self)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
}

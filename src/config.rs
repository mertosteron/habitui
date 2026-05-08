use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const APP_DIR: &str = "habitui";
const FILE_NAME: &str = "config.json";

/// User-facing color theme. Each variant maps to a `Palette` (see views.rs).
/// Persisted to disk so the choice survives across runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    Green,
    Blue,
    Red,
    Yellow,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Green
    }
}

impl Theme {
    pub fn next(self) -> Self {
        match self {
            Theme::Green => Theme::Blue,
            Theme::Blue => Theme::Red,
            Theme::Red => Theme::Yellow,
            Theme::Yellow => Theme::Green,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Theme::Green => "Green",
            Theme::Blue => "Blue",
            Theme::Red => "Red",
            Theme::Yellow => "Yellow",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub theme: Theme,
}

/// Default config location. Sits next to habits.json in the OS data dir.
pub fn config_path() -> PathBuf {
    let base = dirs::data_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join(APP_DIR).join(FILE_NAME)
}

/// Load config from the default path. Missing/invalid file → defaults.
pub fn load() -> Config {
    load_from(&config_path()).unwrap_or_default()
}

pub fn load_from(path: &Path) -> io::Result<Config> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e),
    }
}

pub fn save(c: &Config) -> io::Result<()> {
    save_to(c, &config_path())
}

pub fn save_to(c: &Config, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let bytes = serde_json::to_vec_pretty(c)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let tmp = tmp_sibling(path);
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

fn tmp_sibling(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from("config.json"));
    name.push(".tmp");
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

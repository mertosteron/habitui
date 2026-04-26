use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::data::{HabitStore, STORE_VERSION};

const APP_DIR: &str = "habitui";
const FILE_NAME: &str = "habits.json";

/// Default on-disk location: `<dirs::data_dir()>/habitui/habits.json`.
/// On Linux this honors `XDG_DATA_HOME` (falling back to `~/.local/share`).
pub fn data_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_DIR).join(FILE_NAME)
}

/// Load the store from the default path. Returns an empty store if the file
/// does not exist.
pub fn load() -> io::Result<HabitStore> {
    load_from(&data_path())
}

/// Load the store from `path`. Returns an empty store if the file does not exist.
pub fn load_from(path: &Path) -> io::Result<HabitStore> {
    match fs::read(path) {
        Ok(bytes) => {
            let mut store: HabitStore = serde_json::from_slice(&bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            // Migrate older versions in-memory: serde defaults handle new
            // fields, but we still want the version stamp current so a
            // subsequent save reflects the migrated schema.
            if store.version < STORE_VERSION {
                store.version = STORE_VERSION;
            }
            Ok(store)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(HabitStore::new()),
        Err(e) => Err(e),
    }
}

/// Save the store to the default path atomically.
pub fn save(store: &HabitStore) -> io::Result<()> {
    save_to(store, &data_path())
}

/// Save the store to `path` atomically: write to a sibling `.tmp` file and
/// rename into place. Creates the parent directory if missing.
pub fn save_to(store: &HabitStore, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let bytes = serde_json::to_vec_pretty(store)
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
        .unwrap_or_else(|| std::ffi::OsString::from("habits.json"));
    name.push(".tmp");
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

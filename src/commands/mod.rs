//! One module per command. Shared file helpers live here.

pub mod light;
pub mod profiles;
pub mod set;
pub mod show;
pub mod undo;

use isku_tool::protocol::{profile_index, Report, THUMB_KEYS};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn save(folder: Option<&Path>, name: &str, bytes: &[u8]) -> Result<(), String> {
    match folder {
        Some(folder) => fs::write(folder.join(name), bytes).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

pub fn timestamp() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| e.to_string())
}

pub fn new_backup_folder() -> Result<PathBuf, String> {
    let folder = PathBuf::from("backups").join(format!("backup-{}", timestamp()?));
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    Ok(folder)
}

pub fn has_backup(profile: u8) -> bool {
    let file = format!("profile-{}-{}.bin", profile + 1, THUMB_KEYS.report.name);
    fs::read_dir("backups")
        .is_ok_and(|mut entries| entries.any(|e| e.is_ok_and(|e| e.path().join(&file).exists())))
}

/// Saves a report's bytes from just before a change, so `undo` can put it back.
pub fn save_change(profile: u8, report: &Report, bytes: &[u8]) -> Result<PathBuf, String> {
    let folder = PathBuf::from("backups").join("changes");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let file = folder.join(format!(
        "{}-profile-{}-{}.bin",
        timestamp()?,
        profile + 1,
        report.name
    ));
    fs::write(&file, bytes).map_err(|e| e.to_string())?;
    Ok(file)
}

/// Stops a change when there is no backup of that profile yet.
pub fn require_backup(profile: u8) -> Result<(), String> {
    if has_backup(profile) {
        Ok(())
    } else {
        Err(format!(
            "no backup of profile {} found, run: cargo run -- backup",
            profile + 1
        ))
    }
}

pub fn profile_arg(number: &str) -> Result<u8, String> {
    profile_index(number).ok_or_else(|| "use a profile number, 1 to 5".to_string())
}

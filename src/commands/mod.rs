//! One module per command. Shared file helpers live here.

pub mod light;
pub mod profiles;
pub mod set;
pub mod show;
pub mod undo;

use isku_tool::protocol::{parse_change_name, profile_index, Report, THUMB_KEYS};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn save(folder: Option<&Path>, name: &str, bytes: &[u8]) -> Result<(), String> {
    match folder {
        Some(folder) => fs::write(folder.join(name), bytes).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Milliseconds since 1970. Older files used seconds, which are smaller
/// numbers, so they still sort before newer ones.
pub fn timestamp() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .map_err(|e| e.to_string())
}

/// Makes a new folder that no earlier backup uses, even when two backups
/// start in the same millisecond.
pub fn new_backup_folder() -> Result<PathBuf, String> {
    let backups = PathBuf::from("backups");
    fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
    let mut time = timestamp()?;
    loop {
        let folder = backups.join(format!("backup-{time}"));
        match fs::create_dir(&folder) {
            Ok(()) => return Ok(folder),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => time += 1,
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// A time for the next change file that is later than every change file
/// already saved, undone or not, so `undo` always finds the newest one.
fn next_change_time(folder: &Path) -> Result<u64, String> {
    let last = fs::read_dir(folder)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let name = name.strip_suffix(".undone").unwrap_or(&name);
            parse_change_name(name).map(|(time, _, _)| time)
        })
        .max();
    let now = timestamp()?;
    Ok(match last {
        Some(last) if last >= now => last + 1,
        _ => now,
    })
}

/// The file name of one report in a backup folder.
pub fn backup_file_name(profile: u8, report: &Report) -> String {
    format!("profile-{}-{}.bin", profile + 1, report.name)
}

pub fn has_backup(profile: u8) -> bool {
    let file = backup_file_name(profile, &THUMB_KEYS.report);
    fs::read_dir("backups")
        .is_ok_and(|mut entries| entries.any(|e| e.is_ok_and(|e| e.path().join(&file).exists())))
}

/// Saves a report's bytes from just before a change, so `undo` can put it back.
pub fn save_change(profile: u8, report: &Report, bytes: &[u8]) -> Result<PathBuf, String> {
    let folder = PathBuf::from("backups").join("changes");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let file = folder.join(format!(
        "{}-profile-{}-{}.bin",
        next_change_time(&folder)?,
        profile + 1,
        report.name
    ));
    // create_new: never overwrite the bytes of an earlier change.
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)
        .and_then(|mut f| std::io::Write::write_all(&mut f, bytes))
        .map_err(|e| format!("{}: {e}", file.display()))?;
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

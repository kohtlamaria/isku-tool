//! One module per command. Shared file helpers live here.

pub mod light;
pub mod profiles;
pub mod set;
pub mod show;
pub mod undo;

use isku_tool::device::{check_writable, Isku};
use isku_tool::protocol::{parse_change_name, profile_index, Report, THUMB_KEYS};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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

/// Where `undo` keeps the bytes from before each change.
pub fn changes_folder() -> PathBuf {
    PathBuf::from("backups").join("changes")
}

/// One file in the changes folder.
pub struct Change {
    pub time: u64,
    pub name: String,
    pub profile: u8,
    pub report: Report,
    /// Already used by `undo`, or set aside as invalid (`.bad`).
    pub done: bool,
}

/// Every change file, including undone and invalid ones, so new change
/// files never reuse their names. Other files are ignored.
pub fn list_changes() -> Result<Vec<Change>, String> {
    let entries = match fs::read_dir(changes_folder()) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    Ok(entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let base = name
                .strip_suffix(".undone")
                .or_else(|| name.strip_suffix(".bad"))
                .unwrap_or(&name);
            let done = base.len() != name.len();
            let (time, profile, report) = parse_change_name(base)?;
            Some(Change {
                time,
                name,
                profile,
                report,
                done,
            })
        })
        .collect())
}

/// The last change time used by this run, 0 before the first change.
static LAST_CHANGE_TIME: AtomicU64 = AtomicU64::new(0);

/// A time for the next change file that is later than every change file
/// already saved, undone or not, so `undo` always finds the newest one. The
/// changes folder is listed only once per run; this run's own later files
/// are covered by remembering the last time used.
fn next_change_time() -> Result<u64, String> {
    let mut last = LAST_CHANGE_TIME.load(Ordering::Relaxed);
    if last == 0 {
        last = list_changes()?.iter().map(|c| c.time).max().unwrap_or(0);
    }
    let next = timestamp()?.max(last + 1);
    LAST_CHANGE_TIME.store(next, Ordering::Relaxed);
    Ok(next)
}

/// Writes one report after saving its old bytes, so `undo` can put it back.
/// A report the keyboard would refuse is rejected before anything is saved.
/// If the write itself fails, the change file is kept, because a write that
/// timed out can still land.
pub fn write_with_undo(
    isku: &Isku,
    profile: u8,
    report: &Report,
    current: &[u8],
    updated: &[u8],
) -> Result<PathBuf, String> {
    check_writable(profile, report, updated)?;
    let file = save_change(profile, report, current)?;
    isku.write_profile_report(profile, report, updated)
        .map_err(|e| format!("{e} (the bytes from before are in {})", file.display()))?;
    Ok(file)
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
fn save_change(profile: u8, report: &Report, bytes: &[u8]) -> Result<PathBuf, String> {
    let folder = changes_folder();
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let file = folder.join(format!(
        "{}-profile-{}-{}.bin",
        next_change_time()?,
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

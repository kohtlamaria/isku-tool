//! Automatic cleanup of `backups/`, run after every command that saves files.
//!
//! Only two things are ever deleted:
//! - undone changes (`.undone`) older than 30 days, which `undo` will
//!   never need again
//! - empty `backup-<time>` folders older than a day, left by a `backup` that
//!   failed before it saved anything
//!
//! Full backups are never deleted automatically, and neither are changes
//! that have not been undone or that `undo` set aside as invalid (`.bad`).

use super::{
    backups_folder, changes_folder, list_changes, timestamp, Change, ChangeKind, ChangeState,
    UNDONE_AT_FILE,
};
use std::fs;
use std::path::Path;

const DAY_MS: u64 = 24 * 60 * 60 * 1000;
const USED_CHANGE_AGE_MS: u64 = 30 * DAY_MS;
const EMPTY_BACKUP_AGE_MS: u64 = DAY_MS;

/// Older files were named in seconds, newer ones in milliseconds. Times
/// below this are seconds.
const FIRST_MS_TIME: u64 = 100_000_000_000;

fn as_ms(time: u64) -> u64 {
    if time < FIRST_MS_TIME {
        time.saturating_mul(1000)
    } else {
        time
    }
}

fn older_than(time: u64, age_ms: u64, now_ms: u64) -> bool {
    as_ms(time).saturating_add(age_ms) < now_ms
}

/// Undone changes that were undone longer ago than the age limit. Each comes
/// with the time it was undone, or the time it was made when that is not
/// known. Pending and `.bad` ones are never picked.
fn old_used_changes(changes: &[(Change, u64)], now_ms: u64) -> Vec<&Change> {
    changes
        .iter()
        .filter(|(c, undone_at)| {
            c.state == ChangeState::Undone && older_than(*undone_at, USED_CHANGE_AGE_MS, now_ms)
        })
        .map(|(c, _)| c)
        .collect()
}

/// When a change was undone: the note `undo` leaves in a command folder, or
/// the time the change was made for older single files.
fn undone_at(folder: &Path, change: &Change) -> u64 {
    match (change.state, &change.kind) {
        (ChangeState::Undone, ChangeKind::Command) => {
            fs::read_to_string(folder.join(&change.name).join(UNDONE_AT_FILE))
                .ok()
                .and_then(|text| text.trim().parse().ok())
                .unwrap_or(change.time)
        }
        _ => change.time,
    }
}

/// The time in a backup folder name like `backup-1794087795971`.
fn backup_time(name: &str) -> Option<u64> {
    let digits = name.strip_prefix("backup-")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Deletes old used changes and old empty backup folders. Problems are
/// printed as warnings and never fail the command that ran before it.
pub fn cleanup() {
    let now = match timestamp() {
        Ok(now) => now,
        Err(e) => return eprintln!("Warning: cleanup of backups/ skipped: {e}"),
    };
    let changes = clean_changes(now).unwrap_or_else(|e| {
        eprintln!("Warning: cleanup of backups/changes failed: {e}");
        0
    });
    let folders = clean_empty_backups(now).unwrap_or_else(|e| {
        eprintln!("Warning: cleanup of empty backup folders failed: {e}");
        0
    });
    if changes + folders > 0 {
        println!("Cleaned up {changes} old undone changes and {folders} empty backup folders.");
    }
}

fn clean_changes(now: u64) -> Result<usize, String> {
    let folder = changes_folder();
    let changes: Vec<(Change, u64)> = list_changes()?
        .into_iter()
        .map(|c| {
            let at = undone_at(&folder, &c);
            (c, at)
        })
        .collect();
    let mut removed = 0;
    for change in old_used_changes(&changes, now) {
        let path = folder.join(&change.name);
        let result = match change.kind {
            ChangeKind::Command => fs::remove_dir_all(&path),
            ChangeKind::OldFile { .. } => fs::remove_file(&path),
        };
        match result {
            Ok(()) => removed += 1,
            Err(e) => eprintln!("Warning: could not delete {}: {e}", path.display()),
        }
    }
    Ok(removed)
}

fn clean_empty_backups(now: u64) -> Result<usize, String> {
    let entries = match fs::read_dir(backups_folder()) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.to_string()),
    };
    let mut removed = 0;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let old = backup_time(&name).is_some_and(|t| older_than(t, EMPTY_BACKUP_AGE_MS, now));
        // remove_dir only removes an empty folder; one with files in it
        // fails and stays.
        if old && entry.file_type().is_ok_and(|t| t.is_dir()) && fs::remove_dir(entry.path()).is_ok()
        {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_794_000_000_000;

    /// A change made at `time` and undone at `undone_at`.
    fn change(time: u64, undone_at: u64, state: ChangeState, kind: ChangeKind) -> (Change, u64) {
        let change = Change {
            time,
            name: format!("{time}"),
            kind,
            state,
        };
        (change, undone_at)
    }

    #[test]
    fn only_old_undone_changes_are_deleted() {
        use isku_tool::protocol::LIGHT;
        use ChangeKind::Command;
        use ChangeState::*;
        let old_file = || ChangeKind::OldFile {
            profile: 4,
            report: LIGHT,
        };
        let old = NOW - 31 * DAY_MS;
        let recent = NOW - 29 * DAY_MS;
        let changes = [
            change(old, old, Undone, Command),          // undone long ago: delete
            change(old + 1, old + 1, Pending, Command), // pending: keep
            change(old + 2, old + 2, Bad, Command),     // set aside: keep
            change(old + 3, recent, Undone, Command),   // old, but undone lately: keep
            change(old / 1000, old / 1000, Undone, old_file()), // old, in seconds: delete
            change(recent / 1000, recent / 1000, Undone, old_file()), // recent, in seconds: keep
            change(u64::MAX, u64::MAX, Undone, old_file()), // no overflow: keep
        ];
        let picked: Vec<_> = old_used_changes(&changes, NOW)
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(picked, [changes[0].0.name.as_str(), changes[4].0.name.as_str()]);
    }

    #[test]
    fn backup_folder_names() {
        assert_eq!(backup_time("backup-1794087795971"), Some(1794087795971));
        assert_eq!(backup_time("backup-1793898236"), Some(1793898236));
        for bad in ["backup-", "backup-12x", "changes", "backup-99999999999999999999"] {
            assert_eq!(backup_time(bad), None, "{bad}");
        }
        assert!(older_than(1793898236, DAY_MS, NOW));
        assert!(!older_than(NOW - 1000, DAY_MS, NOW));
    }
}

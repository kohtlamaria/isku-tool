//! One module per command. Shared file helpers live here.

pub mod cleanup;
pub mod light;
pub mod macros;
pub mod profiles;
pub mod set;
pub mod show;
pub mod undo;

use isku_tool::device::{check_macro_writable, check_writable, Isku};
use isku_tool::protocol::{
    empty_macro_file_name, macro_file_name, parse_change_name, profile_index, valid_macro_report,
    Report, THUMB_KEYS,
};
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
    create_unique_folder(&backups_folder(), timestamp()?, |time| {
        format!("backup-{time}")
    })
}

/// Makes a new folder in `parent` named by `name(time)`, starting at `time`
/// and stepping the time forward while that name is taken.
fn create_unique_folder(
    parent: &Path,
    mut time: u64,
    name: impl Fn(u64) -> String,
) -> Result<PathBuf, String> {
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    loop {
        let folder = parent.join(name(time));
        match fs::create_dir(&folder) {
            Ok(()) => return Ok(folder),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && time < u64::MAX => time += 1,
            Err(e) => return Err(format!("{}: {e}", folder.display())),
        }
    }
}

/// Where all backups and change files are kept.
pub fn backups_folder() -> PathBuf {
    PathBuf::from("backups")
}

/// Where `undo` keeps the bytes from before each change.
pub fn changes_folder() -> PathBuf {
    backups_folder().join("changes")
}

/// One entry in the changes folder.
pub struct Change {
    pub time: u64,
    pub name: String,
    pub kind: ChangeKind,
    pub state: ChangeState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeState {
    /// Not undone yet; `undo` will use it.
    Pending,
    /// Used by `undo` (`.undone`). Only these are ever cleaned up.
    Undone,
    /// Set aside by `undo` as invalid (`.bad`). Kept for a look.
    Bad,
}

/// The file `undo` leaves in a command's change folder, holding the time it
/// was undone, so cleanup counts the 30 days from then.
pub const UNDONE_AT_FILE: &str = "undone-at.txt";

pub enum ChangeKind {
    /// A folder like `1794141018978-reset-5` with the bytes from before
    /// every report one command wrote.
    Command,
    /// A single file like `1794141018978-profile-5-light.bin`, from before
    /// changes were grouped by command.
    OldFile { profile: u8, report: Report },
}

/// Reads a command's change folder name like `1794141018978-reset-5` into
/// its time.
fn parse_command_change_name(name: &str) -> Option<u64> {
    let (time, label) = name.split_once('-')?;
    let label_ok = !label.is_empty()
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    let time_ok = !time.is_empty() && time.bytes().all(|b| b.is_ascii_digit());
    if label_ok && time_ok {
        time.parse().ok()
    } else {
        None
    }
}

/// Every change, including undone and invalid ones. Other files are ignored.
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
            let (base, state) = if let Some(base) = name.strip_suffix(".undone") {
                (base, ChangeState::Undone)
            } else if let Some(base) = name.strip_suffix(".bad") {
                (base, ChangeState::Bad)
            } else {
                (name.as_str(), ChangeState::Pending)
            };
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            let (time, kind) = if is_dir {
                (parse_command_change_name(base)?, ChangeKind::Command)
            } else {
                let (time, profile, report) = parse_change_name(base)?;
                (time, ChangeKind::OldFile { profile, report })
            };
            Some(Change {
                time,
                name,
                kind,
                state,
            })
        })
        .collect())
}

/// The bytes from before every report one command writes, kept together in
/// one folder so `undo` can put the whole command back at once. The folder
/// is made on the first write, so a command that changes nothing leaves
/// nothing behind.
pub struct ChangeSet {
    label: String,
    folder: Option<PathBuf>,
    /// True once a report was sent to the keyboard, so the profile may have
    /// changed.
    sent: bool,
}

impl ChangeSet {
    /// `label` names the command, such as `reset-5`.
    pub fn new(label: &str) -> Self {
        ChangeSet {
            label: label.to_string(),
            folder: None,
            sent: false,
        }
    }

    fn open_folder(&mut self) -> Result<PathBuf, String> {
        if let Some(folder) = &self.folder {
            return Ok(folder.clone());
        }
        // Later than every change so far, so `undo` finds this one first.
        let last = list_changes()?.iter().map(|c| c.time).max().unwrap_or(0);
        let time = timestamp()?.max(last.saturating_add(1));
        let label = &self.label;
        let folder =
            create_unique_folder(&changes_folder(), time, |time| format!("{time}-{label}"))?;
        self.folder = Some(folder.clone());
        Ok(folder)
    }

    /// Saves the bytes from before a change. They go to a temporary file
    /// first and are renamed into place, so a saved report is either
    /// complete or not there at all. If one command writes the same report
    /// twice, the first, oldest bytes are kept.
    fn save_old_bytes(&mut self, file_name: &str, current: &[u8]) -> Result<(), String> {
        let folder = self.open_folder()?;
        let file = folder.join(file_name);
        if file.exists() {
            return Ok(());
        }
        let temporary = folder.join(format!("{file_name}.tmp"));
        let saved = fs::write(&temporary, current).and_then(|()| fs::rename(&temporary, &file));
        if let Err(e) = saved {
            let _ = fs::remove_file(&temporary);
            // Nothing was sent yet, so an empty folder is not a change.
            if !self.sent && fs::remove_dir(&folder).is_ok() {
                self.folder = None;
            }
            return Err(format!("{}: {e}", file.display()));
        }
        Ok(())
    }

    /// Writes one report after saving its old bytes. A report the keyboard
    /// would refuse is rejected before anything is saved. If the write
    /// itself fails, the saved bytes are kept, because a write that timed
    /// out can still land, and the error says how to put the profile back.
    pub fn write(
        &mut self,
        isku: &Isku,
        profile: u8,
        report: &Report,
        current: &[u8],
        updated: &[u8],
    ) -> Result<(), String> {
        check_writable(profile, report, updated).map_err(|e| self.with_undo_note(e, profile))?;
        self.save_old_bytes(&backup_file_name(profile, report), current)
            .map_err(|e| self.with_undo_note(e, profile))?;
        self.sent = true;
        isku.write_profile_report(profile, report, updated)
            .map_err(|e| self.with_undo_note(e, profile))
    }

    /// Writes one macro after saving the one it replaces, the same way as
    /// `write`. A slot that held no macro is noted with an empty `.empty`
    /// file, since there are no old bytes to save. A slot whose macro
    /// `undo` could not put back is refused before anything is saved.
    pub fn write_macro(
        &mut self,
        isku: &Isku,
        profile: u8,
        key_index: u8,
        current: Option<&[u8]>,
        updated: &[u8],
    ) -> Result<(), String> {
        check_macro_writable(profile, key_index, updated)
            .map_err(|e| self.with_undo_note(e, profile))?;
        check_undoable_macro(profile, key_index, current)
            .map_err(|e| self.with_undo_note(e, profile))?;
        let full = macro_file_name(profile, key_index);
        let empty = empty_macro_file_name(profile, key_index);
        // Keep the first, oldest state of the slot, whichever file holds it.
        let saved = if self.has_saved(&full) || self.has_saved(&empty) {
            Ok(())
        } else {
            match current {
                Some(bytes) => self.save_old_bytes(&full, bytes),
                None => self.save_old_bytes(&empty, &[]),
            }
        };
        saved.map_err(|e| self.with_undo_note(e, profile))?;
        self.sent = true;
        isku.write_macro(profile, key_index, updated)
            .map_err(|e| self.with_undo_note(e, profile))
    }

    /// True when this change already saved a file with this name.
    fn has_saved(&self, file_name: &str) -> bool {
        self.folder
            .as_ref()
            .is_some_and(|folder| folder.join(file_name).exists())
    }

    /// Adds how to put things back to an error, once a report was sent to
    /// the keyboard during this command.
    pub fn with_undo_note(&self, error: String, profile: u8) -> String {
        if self.sent {
            format!(
                "{error}\nProfile {} may be partly changed. Put it back with: cargo run -- undo",
                profile + 1
            )
        } else {
            error
        }
    }

    /// Prints how to put this command's changes back, if it changed anything.
    pub fn print_undo_hint(&self) {
        if let Some(folder) = &self.folder {
            println!(
                "Before this change: {}. Undo with: cargo run -- undo",
                folder.display()
            );
        }
    }
}

/// Refuses to replace a macro that `undo` could not put back, because it
/// is not laid out the way the tool expects.
pub fn check_undoable_macro(
    profile: u8,
    key_index: u8,
    current: Option<&[u8]>,
) -> Result<(), String> {
    if current.is_some_and(|bytes| !valid_macro_report(profile, key_index, bytes)) {
        return Err(format!(
            "profile {}: the macro in slot 0x{key_index:02x} has an unexpected layout, so \
             undo could not put it back. Nothing was written for it.",
            profile + 1
        ));
    }
    Ok(())
}

/// The file name of one report in a backup folder.
pub fn backup_file_name(profile: u8, report: &Report) -> String {
    format!("profile-{}-{}.bin", profile + 1, report.name)
}

pub fn has_backup(profile: u8) -> bool {
    let file = backup_file_name(profile, &THUMB_KEYS.report);
    fs::read_dir(backups_folder())
        .is_ok_and(|mut entries| entries.any(|e| e.is_ok_and(|e| e.path().join(&file).exists())))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_change_names() {
        assert_eq!(
            parse_command_change_name("1794141018978-reset-5"),
            Some(1794141018978)
        );
        assert_eq!(parse_command_change_name("17-copy-1-to-5"), Some(17));
        for bad in [
            "1794141018978-profile-5-light.bin",
            "1794141018978-",
            "-reset-5",
            "12x-reset-5",
            "1794141018978-Reset",
            "notes",
        ] {
            assert_eq!(parse_command_change_name(bad), None, "{bad}");
        }
    }
}

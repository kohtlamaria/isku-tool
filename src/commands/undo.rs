//! `undo`: put back the last change made by `set`, `light`, `reset`,
//! `restore`, `copy` or `macro`. A command's whole change goes back in one
//! run.

use super::{
    changes_folder, list_changes, timestamp, Change, ChangeKind, ChangeState, UNDONE_AT_FILE,
};
use isku_tool::device::Isku;
use isku_tool::protocol::{
    empty_macro_report, parse_macro_file_name, parse_report_file_name, valid_macro_report,
    valid_profile_report, write_order, Report,
};
use std::fs;
use std::path::Path;

/// What one saved file puts back, with the bytes from before the change.
enum Target {
    Report(Report, Vec<u8>),
    /// A macro slot. `None` means the slot held no macro before.
    Macro(u8, Option<Vec<u8>>),
}

/// One saved report or macro and the profile it belongs to.
struct Saved {
    profile: u8,
    target: Target,
}

impl Saved {
    /// Where this comes in the order commands write: macros first, so a key
    /// never plays a half-written slot, then reports in PROFILE_REPORTS
    /// order. `undo` goes the other way.
    fn order(&self) -> usize {
        match &self.target {
            Target::Macro(..) => 0,
            Target::Report(report, _) => 1 + write_order(report),
        }
    }

    fn describe(&self) -> String {
        match &self.target {
            Target::Report(report, _) => report.name.to_string(),
            Target::Macro(key_index, _) => format!("macro 0x{key_index:02x}"),
        }
    }

    fn was_empty_slot(&self) -> bool {
        matches!(self.target, Target::Macro(_, None))
    }
}

/// Why a change could not be read.
enum ReadError {
    /// The files could not be read right now. Nothing is wrong with the
    /// change itself, so it is left as it is.
    Io(String),
    /// A saved report has wrong contents, so the change cannot be used.
    Invalid(String),
}

/// Reads and checks every saved report and macro in a command's change
/// folder. Other files, such as `desktop.ini` or a leftover `.tmp`, are
/// ignored. They come back in the reverse of the order commands write them.
fn read_command_folder(folder: &Path) -> Result<Vec<Saved>, ReadError> {
    let io = |e: std::io::Error| ReadError::Io(format!("{}: {e}", folder.display()));
    let mut saved = Vec::new();
    for entry in fs::read_dir(folder).map_err(io)? {
        let entry = entry.map_err(io)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some((profile, report)) = parse_report_file_name(&name) {
            saved.push(read_saved(&entry.path(), profile, report)?);
        } else if let Some((profile, key_index, empty)) = parse_macro_file_name(&name) {
            saved.push(read_saved_macro(&entry.path(), profile, key_index, empty)?);
        }
    }
    saved.sort_by_key(|s| std::cmp::Reverse(s.order()));
    Ok(saved)
}

fn read_saved(path: &Path, profile: u8, report: Report) -> Result<Saved, ReadError> {
    let bytes = fs::read(path).map_err(|e| ReadError::Io(format!("{}: {e}", path.display())))?;
    if !valid_profile_report(&report, profile, &bytes) {
        return Err(ReadError::Invalid(format!(
            "{} is not a valid saved report",
            path.display()
        )));
    }
    Ok(Saved {
        profile,
        target: Target::Report(report, bytes),
    })
}

fn read_saved_macro(
    path: &Path,
    profile: u8,
    key_index: u8,
    empty: bool,
) -> Result<Saved, ReadError> {
    let bytes = if empty {
        None
    } else {
        let bytes =
            fs::read(path).map_err(|e| ReadError::Io(format!("{}: {e}", path.display())))?;
        if !valid_macro_report(profile, key_index, &bytes) {
            return Err(ReadError::Invalid(format!(
                "{} is not a valid saved macro",
                path.display()
            )));
        }
        Some(bytes)
    };
    Ok(Saved {
        profile,
        target: Target::Macro(key_index, bytes),
    })
}

fn read_change(folder: &Path, change: &Change) -> Result<Vec<Saved>, ReadError> {
    let path = folder.join(&change.name);
    match change.kind {
        ChangeKind::Command => read_command_folder(&path),
        ChangeKind::OldFile { profile, report } => Ok(vec![read_saved(&path, profile, report)?]),
    }
}

/// Adds how to finish to an error, once part of a change was sent back.
fn finish_note(error: String, written: usize) -> String {
    if written > 0 {
        format!("{error}\nThis change is partly put back. Run undo again to finish it.")
    } else {
        error
    }
}

/// Marks a change as undone: notes when, then renames it to `.undone`.
fn mark_undone(folder: &Path, change: &Change) -> Result<(), String> {
    let path = folder.join(&change.name);
    if let ChangeKind::Command = change.kind {
        fs::write(path.join(UNDONE_AT_FILE), timestamp()?.to_string())
            .map_err(|e| e.to_string())?;
    }
    fs::rename(&path, folder.join(format!("{}.undone", change.name))).map_err(|e| e.to_string())
}

/// Puts one saved item back unless the keyboard already has it. Returns
/// whether anything was sent. `written` counts sends for error notes and is
/// raised before each send, because a failed write can still have landed.
fn put_back(isku: &Isku, item: &Saved, written: &mut usize) -> Result<bool, String> {
    match &item.target {
        Target::Report(report, bytes) => {
            if isku.read_profile_report(item.profile, report)? == *bytes {
                return Ok(false);
            }
            *written += 1;
            isku.write_profile_report(item.profile, report, bytes)?;
        }
        Target::Macro(key_index, saved) => {
            let key_index = *key_index;
            let now = isku.read_macro(item.profile, key_index)?;
            let bytes = match saved {
                Some(bytes) => bytes.clone(),
                // Still empty, or already holding nothing: done.
                None if now.is_none() => return Ok(false),
                // A slot cannot be emptied again, so it gets a macro that
                // plays nothing.
                None => empty_macro_report(item.profile, key_index).ok_or("bad macro slot")?,
            };
            if now.as_deref() == Some(bytes.as_slice()) {
                return Ok(false);
            }
            *written += 1;
            isku.write_macro(item.profile, key_index, &bytes)?;
        }
    }
    Ok(true)
}

/// Puts back the most recent change that has not been undone yet, and
/// renames it to `.undone`. If a write fails partway, the change stays as it
/// is, and running `undo` again finishes it: items that are already back
/// are skipped. Change folders with nothing saved in them, which an
/// interrupted command can leave, are marked undone and skipped.
pub fn undo() -> Result<(), String> {
    let folder = changes_folder();
    let mut pending: Vec<Change> = list_changes()?
        .into_iter()
        .filter(|c| c.state == ChangeState::Pending)
        .collect();
    pending.sort_by(|a, b| (b.time, &b.name).cmp(&(a.time, &a.name)));

    let mut chosen = None;
    for change in pending {
        match read_change(&folder, &change) {
            Ok(saved) if saved.is_empty() => {
                mark_undone(&folder, &change)?;
            }
            Ok(saved) => {
                chosen = Some((change, saved));
                break;
            }
            Err(ReadError::Io(error)) => {
                return Err(format!("{error}\nNothing was changed. Try undo again."));
            }
            Err(ReadError::Invalid(error)) => {
                // Set it aside, so the next `undo` can reach older changes.
                // It is kept for a look, and cleanup never deletes it.
                let bad = folder.join(format!("{}.bad", change.name));
                fs::rename(folder.join(&change.name), &bad).map_err(|e| e.to_string())?;
                return Err(format!(
                    "{error}\nNothing was changed. Set it aside as {}. Run undo again for the change before it.",
                    bad.display()
                ));
            }
        }
    }
    let (change, saved) = chosen.ok_or("no changes to undo")?;

    let isku = Isku::open()?;
    let mut written = 0;
    for item in &saved {
        if put_back(&isku, item, &mut written).map_err(|e| finish_note(e, written))? {
            println!(
                "Profile {}: {} put back. Read back and confirmed.",
                item.profile + 1,
                item.describe()
            );
            if item.was_empty_slot() {
                println!(
                    "  The slot held no macro before, so it now holds one that plays nothing."
                );
            }
        }
    }
    if written == 0 {
        println!("The keyboard already matches, nothing to write.");
    }
    mark_undone(&folder, &change)?;
    println!("Undid {}", change.name);
    Ok(())
}

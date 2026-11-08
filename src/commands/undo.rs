//! `undo`: put back the last change made by `set`, `light`, `reset`,
//! `restore` or `copy`. A command's whole change goes back in one run.

use super::{changes_folder, list_changes, timestamp, Change, ChangeKind, ChangeState, UNDONE_AT_FILE};
use isku_tool::device::Isku;
use isku_tool::protocol::{parse_report_file_name, valid_profile_report, write_order, Report};
use std::fs;
use std::path::Path;

/// One saved report: where it goes and the bytes from before the change.
struct Saved {
    profile: u8,
    report: Report,
    bytes: Vec<u8>,
}

/// Why a change could not be read.
enum ReadError {
    /// The files could not be read right now. Nothing is wrong with the
    /// change itself, so it is left as it is.
    Io(String),
    /// A saved report has wrong contents, so the change cannot be used.
    Invalid(String),
}

/// Reads and checks every saved report in a command's change folder. Other
/// files, such as `desktop.ini` or a leftover `.tmp`, are ignored. The
/// reports come back in the reverse of the order commands write them.
fn read_command_folder(folder: &Path) -> Result<Vec<Saved>, ReadError> {
    let io = |e: std::io::Error| ReadError::Io(format!("{}: {e}", folder.display()));
    let mut saved = Vec::new();
    for entry in fs::read_dir(folder).map_err(io)? {
        let entry = entry.map_err(io)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((profile, report)) = parse_report_file_name(&name) else {
            continue;
        };
        saved.push(read_saved(&entry.path(), profile, report)?);
    }
    saved.sort_by_key(|s| std::cmp::Reverse(write_order(&s.report)));
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
        report,
        bytes,
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

/// Puts back the most recent change that has not been undone yet, and
/// renames it to `.undone`. If a write fails partway, the change stays as it
/// is, and running `undo` again finishes it: reports that are already back
/// are skipped. Change folders with no saved reports in them, which an
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
        let now = isku
            .read_profile_report(item.profile, &item.report)
            .map_err(|e| finish_note(e, written))?;
        if now == item.bytes {
            continue;
        }
        // Counted before the write: a failed write can still have landed.
        written += 1;
        isku.write_profile_report(item.profile, &item.report, &item.bytes)
            .map_err(|e| finish_note(e, written))?;
        println!(
            "Profile {}: {} put back. Read back and confirmed.",
            item.profile + 1,
            item.report.name
        );
    }
    if written == 0 {
        println!("The keyboard already matches, nothing to write.");
    }
    mark_undone(&folder, &change)?;
    println!("Undid {}", change.name);
    Ok(())
}

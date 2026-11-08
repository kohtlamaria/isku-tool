//! `undo`: put back the last report written by `set`, `light`, `reset`,
//! `restore` or `copy`, one report per run.

use super::{changes_folder, list_changes};
use isku_tool::device::Isku;
use isku_tool::protocol::valid_profile_report;
use std::fs;

/// Puts back the most recent change that has not been undone yet, and
/// renames its file to `.undone`. Exactly one change file is used per run.
pub fn undo() -> Result<(), String> {
    let folder = changes_folder();
    let newest = list_changes()?
        .into_iter()
        .filter(|c| !c.done)
        .max_by(|a, b| (a.time, &a.name).cmp(&(b.time, &b.name)))
        .ok_or("no changes to undo")?;
    let path = folder.join(&newest.name);
    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !valid_profile_report(&newest.report, newest.profile, &bytes) {
        // Move it aside, so the next `undo` can reach older changes.
        let bad = folder.join(format!("{}.bad", newest.name));
        fs::rename(&path, &bad).map_err(|e| e.to_string())?;
        return Err(format!(
            "{} is not a valid saved report, renamed it to {}. Run undo again for the change before it.",
            path.display(),
            bad.display()
        ));
    }

    let (profile, report) = (newest.profile, newest.report);
    let isku = Isku::open()?;
    if isku.read_profile_report(profile, &report)? == bytes {
        println!(
            "Profile {}: {} already matches, nothing to write.",
            profile + 1,
            report.name
        );
    } else {
        isku.write_profile_report(profile, &report, &bytes)?;
        println!(
            "Profile {}: {} put back. Read back and confirmed.",
            profile + 1,
            report.name
        );
    }
    let done = folder.join(format!("{}.undone", newest.name));
    fs::rename(&path, &done).map_err(|e| e.to_string())?;
    println!("Undid {}", newest.name);
    Ok(())
}

//! `undo`: put back the last report written by `set`, `light`, `reset` or
//! `restore`, one report per run.

use isku_tool::device::Isku;
use isku_tool::protocol::{parse_change_name, valid_profile_report};
use std::fs;
use std::path::PathBuf;

/// Puts back the most recent change that has not been undone yet. The used change file is renamed to `.undone`.
pub fn undo() -> Result<(), String> {
    let folder = PathBuf::from("backups").join("changes");
    let newest = fs::read_dir(&folder)
        .map_err(|_| "no changes to undo".to_string())?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            parse_change_name(&name).map(|(time, profile, report)| (time, name, profile, report))
        })
        .max_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)))
        .ok_or("no changes to undo")?;
    let (_, name, profile, report) = newest;
    let path = folder.join(&name);
    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !valid_profile_report(&report, profile, &bytes) {
        return Err(format!("{} is not a valid saved report", path.display()));
    }

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
    let done = folder.join(format!("{name}.undone"));
    fs::rename(&path, &done).map_err(|e| e.to_string())?;
    println!("Undid {name}");
    Ok(())
}

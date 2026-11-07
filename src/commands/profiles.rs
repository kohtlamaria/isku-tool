//! Whole-profile commands: `restore`, `reset` and `activate`.

use super::{backup_file_name, new_backup_folder, profile_arg, save, save_change};
use isku_tool::device::Isku;
use isku_tool::protocol::{
    describe_light, Report, valid_profile_report, with_all_defaults, with_capslock, with_light,
    CAPSLOCK_DEFAULT, EASYZONE_KEYS, FUNCTION_KEYS, KEYS_CAPSLOCK, LIGHT, LIGHT_DEFAULT,
    MEDIA_KEYS, PROFILE_REPORTS,
};
use std::fs;
use std::path::Path;

pub fn restore(folder: &str, profile: &str) -> Result<(), String> {
    let profile = profile_arg(profile)?;
    let folder = Path::new(folder);
    let mut backups = Vec::new();
    for report in &PROFILE_REPORTS {
        let path = folder.join(backup_file_name(profile, report));
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if !valid_profile_report(report, profile, &bytes) {
            return Err(format!("{} is not a valid backup", path.display()));
        }
        backups.push((report, bytes));
    }

    let isku = Isku::open()?;
    let mut restored = 0;
    for (report, bytes) in &backups {
        let current = isku.read_profile_report(profile, report)?;
        if current != *bytes {
            write_with_undo(&isku, profile, report, &current, bytes)?;
            println!("Restored {}", report.name);
            restored += 1;
        }
    }
    println!(
        "Profile {} matches the backup. Reports restored: {restored}",
        profile + 1
    );
    Ok(())
}

pub fn reset(number: &str) -> Result<(), String> {
    let profile = profile_arg(number)?;
    let isku = Isku::open()?;

    // Save the whole profile first, so `restore` can undo the reset.
    // The same bytes are reused below, so each report is read only once.
    let folder = new_backup_folder()?;
    let mut saved = Vec::new();
    for report in &PROFILE_REPORTS {
        let bytes = isku.read_profile_report(profile, report)?;
        save(Some(&folder), &backup_file_name(profile, report), &bytes)?;
        saved.push((report.id, bytes));
    }
    println!("Saved profile {} to {}", profile + 1, folder.display());
    let current_of = |report: &Report| -> Result<Vec<u8>, String> {
        saved
            .iter()
            .find(|(id, _)| *id == report.id)
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| format!("{} was not read", report.name))
    };

    let mut changed = 0;
    for group in [&EASYZONE_KEYS, &FUNCTION_KEYS, &MEDIA_KEYS] {
        let current = current_of(&group.report)?;
        let updated = with_all_defaults(&current, group).ok_or("bad key report")?;
        if updated != current {
            write_with_undo(&isku, profile, &group.report, &current, &updated)?;
            println!("Reset {}", group.title);
            changed += 1;
        }
    }
    let current = current_of(&KEYS_CAPSLOCK)?;
    let updated = with_capslock(&current, CAPSLOCK_DEFAULT).ok_or("bad Caps Lock report")?;
    if updated != current {
        write_with_undo(&isku, profile, &KEYS_CAPSLOCK, &current, &updated)?;
        println!("Reset Caps Lock to Easy-Shift");
        changed += 1;
    }
    let current = current_of(&LIGHT)?;
    let updated = with_light(&current, LIGHT_DEFAULT).ok_or("bad light report")?;
    if updated != current {
        write_with_undo(&isku, profile, &LIGHT, &current, &updated)?;
        println!("Reset lighting to {}", describe_light(LIGHT_DEFAULT));
        changed += 1;
    }

    println!(
        "Profile {}: {changed} groups reset. Thumb and M keys left as they were.",
        profile + 1
    );
    println!(
        "Undo with: cargo run -- restore {} {}",
        folder.display(),
        profile + 1
    );
    Ok(())
}

/// Writes one report and saves its old bytes first, so `undo` steps back
/// through reset and restore too, in the order the writes happened.
fn write_with_undo(
    isku: &Isku,
    profile: u8,
    report: &Report,
    current: &[u8],
    updated: &[u8],
) -> Result<(), String> {
    save_change(profile, report, current)?;
    isku.write_profile_report(profile, report, updated)
}

pub fn activate(number: &str) -> Result<(), String> {
    let profile = profile_arg(number)?;
    Isku::open()?.activate_profile(profile)?;
    println!("Active profile is now {}", profile + 1);
    Ok(())
}

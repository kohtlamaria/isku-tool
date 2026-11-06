//! Whole-profile commands: `restore`, `reset` and `activate`.

use super::{new_backup_folder, profile_arg, save};
use isku_tool::device::Isku;
use isku_tool::protocol::{
    describe_light, valid_profile_report, with_all_defaults, with_capslock, with_light,
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
        let path = folder.join(format!("profile-{}-{}.bin", profile + 1, report.name));
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if !valid_profile_report(report, profile, &bytes) {
            return Err(format!("{} is not a valid backup", path.display()));
        }
        backups.push((report, bytes));
    }

    let isku = Isku::open()?;
    let mut restored = 0;
    for (report, bytes) in &backups {
        if isku.read_profile_report(profile, report)? != *bytes {
            isku.write_profile_report(profile, report, bytes)?;
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
    let folder = new_backup_folder()?;
    for report in &PROFILE_REPORTS {
        let bytes = isku.read_profile_report(profile, report)?;
        save(
            Some(&folder),
            &format!("profile-{}-{}.bin", profile + 1, report.name),
            &bytes,
        )?;
    }
    println!("Saved profile {} to {}", profile + 1, folder.display());

    let mut changed = 0;
    for group in [&EASYZONE_KEYS, &FUNCTION_KEYS, &MEDIA_KEYS] {
        let current = isku.read_profile_report(profile, &group.report)?;
        let updated = with_all_defaults(&current, group).ok_or("bad key report")?;
        if updated != current {
            isku.write_profile_report(profile, &group.report, &updated)?;
            println!("Reset {}", group.title);
            changed += 1;
        }
    }
    let current = isku.read_profile_report(profile, &KEYS_CAPSLOCK)?;
    let updated = with_capslock(&current, CAPSLOCK_DEFAULT).ok_or("bad Caps Lock report")?;
    if updated != current {
        isku.write_profile_report(profile, &KEYS_CAPSLOCK, &updated)?;
        println!("Reset Caps Lock to Easy-Shift");
        changed += 1;
    }
    let current = isku.read_profile_report(profile, &LIGHT)?;
    let updated = with_light(&current, LIGHT_DEFAULT).ok_or("bad light report")?;
    if updated != current {
        isku.write_profile_report(profile, &LIGHT, &updated)?;
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

pub fn activate(number: &str) -> Result<(), String> {
    let profile = profile_arg(number)?;
    Isku::open()?.activate_profile(profile)?;
    println!("Active profile is now {}", profile + 1);
    Ok(())
}

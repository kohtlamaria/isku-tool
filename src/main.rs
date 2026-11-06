//! cargo run                         show all 5 profiles
//! cargo run -- backup               show them and save them to backups/
//! cargo run -- set 2 T1 7           set a thumb key on profile 2
//! cargo run -- set 2 T1 Ctrl+F5     keys can be letters, F-keys, Space and so on
//! cargo run -- set 2 Shift+Q Ctrl+1  Easy-Shift zone keys and M keys work the same way
//! cargo run -- set 2 F5 Ctrl+1      F1-F12 and media keys (Mute, VolUp...) too
//! cargo run -- set 2 Shift+Q default put a zone, F- or media key back to normal
//! cargo run -- restore <folder> 2   put profile 2 back from a backup
//! cargo run -- reset 2             put profile 2's keys back to factory settings
//! cargo run -- activate <1-5>       switch the active profile

use isku_tool::device::Isku;
use isku_tool::protocol::{
    bindings, describe, describe_macro, find_key, parse_macro, parse_shortcut, profile_index,
    shortcut_text, valid_profile_report, with_all_defaults, with_capslock, with_default, with_shortcut, CAPSLOCK_DEFAULT, KEYS_CAPSLOCK, KeyGroup, ACTION_MACRO, FUNCTION_KEYS, MEDIA_KEYS,
    EASYZONE_KEYS, INFO, LAST_SET, MACRO_KEYS, PROFILE_COUNT, PROFILE_REPORTS, THUMB_KEYS,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn save(folder: Option<&Path>, name: &str, bytes: &[u8]) -> Result<(), String> {
    match folder {
        Some(folder) => fs::write(folder.join(name), bytes).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

fn show_group(
    isku: &Isku,
    profile: u8,
    group: &KeyGroup,
    only_changed: bool,
    folder: Option<&Path>,
) -> Result<usize, String> {
    let report = isku.read_profile_report(profile, &group.report)?;
    let keys = bindings(&report, group.labels.len()).ok_or("key report too short")?;
    println!("  {}", group.title);
    let mut shown = 0;
    for (i, (label, binding)) in group.labels.iter().zip(&keys).enumerate() {
        let key_index = group.first_macro_index + i as u8;
        let mut detail = describe(*binding);
        if binding.action == ACTION_MACRO {
            let bytes = isku.read_macro(profile, key_index)?;
            save(folder, &format!("profile-{}-macro-{key_index:02x}.bin", profile + 1), &bytes)?;
            if let Some(value) = parse_macro(&bytes) {
                detail = format!("macro {}", describe_macro(&value));
            }
        }
        let unchanged = group.defaults.is_some_and(|d| binding.action == d[i]);
        if !(only_changed && unchanged) {
            println!("    {label:9} {detail}");
            shown += 1;
        }
    }
    if only_changed && shown == 0 {
        println!("    nothing changed from default");
    }
    Ok(shown)
}

fn new_backup_folder() -> Result<PathBuf, String> {
    let folder = PathBuf::from("backups").join(format!("backup-{}", timestamp()?));
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    Ok(folder)
}

fn show(folder: Option<&Path>) -> Result<(), String> {
    let isku = Isku::open()?;
    let active_before = isku.active_profile()?;

    for profile in 0..PROFILE_COUNT {
        let marker = if profile == active_before { " (active)" } else { "" };
        println!("Profile {}{marker}", profile + 1);
        show_group(&isku, profile, &THUMB_KEYS, false, folder)?;
        show_group(&isku, profile, &MACRO_KEYS, false, folder)?;
        show_group(&isku, profile, &EASYZONE_KEYS, true, folder)?;
        show_group(&isku, profile, &FUNCTION_KEYS, true, folder)?;
        show_group(&isku, profile, &MEDIA_KEYS, true, folder)?;
        for report in &PROFILE_REPORTS {
            let bytes = isku.read_profile_report(profile, report)?;
            save(folder, &format!("profile-{}-{}.bin", profile + 1, report.name), &bytes)?;
        }
    }

    for report in [&INFO, &LAST_SET] {
        save(folder, &format!("{}.bin", report.name), &isku.read_global(report)?)?;
    }

    let active_after = isku.active_profile()?;
    if active_after != active_before {
        return Err(format!(
            "active profile changed from {} to {}",
            active_before + 1,
            active_after + 1
        ));
    }
    println!("Active profile unchanged: {}", active_after + 1);
    if let Some(folder) = folder {
        println!("Backup saved to {}", folder.display());
    }
    Ok(())
}

fn has_backup(profile: u8) -> bool {
    let file = format!("profile-{}-{}.bin", profile + 1, THUMB_KEYS.report.name);
    fs::read_dir("backups").is_ok_and(|mut entries| {
        entries.any(|e| e.is_ok_and(|e| e.path().join(&file).exists()))
    })
}

fn timestamp() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| e.to_string())
}

fn set_key(profile: &str, label: &str, value: &str) -> Result<(), String> {
    let profile = profile_index(profile).ok_or("use a profile number, 1 to 5")?;
    let (group, position) = find_key(label).ok_or(
        "use T1-T3, M1-M5, F1-F12, Mute, VolDown, VolUp, Play, Prev, Next, WWW, Computer, \
         or with Easy-Shift: Shift+T1, Shift+M1, Shift+Q and the rest of the zone",
    )?;
    let reset = value.eq_ignore_ascii_case("default");
    if reset && group.defaults.is_none() {
        return Err("default only works for zone, F- and media keys, use restore for the others".into());
    }
    let shortcut = if reset {
        None
    } else {
        Some(parse_shortcut(value).ok_or(
            "use a key like 7, Q, F5, Space or Num1, optionally with Ctrl+, Shift+, Alt+ or Win+",
        )?)
    };
    if !has_backup(profile) {
        return Err(format!("no backup of profile {} found, run: cargo run -- backup", profile + 1));
    }

    let isku = Isku::open()?;
    let current = isku.read_profile_report(profile, &group.report)?;

    // Keep the exact bytes from just before this change, so it can be undone.
    let undo = PathBuf::from("backups").join("changes");
    fs::create_dir_all(&undo).map_err(|e| e.to_string())?;
    let undo_file = undo.join(format!(
        "{}-profile-{}-{}.bin",
        timestamp()?,
        profile + 1,
        group.report.name
    ));
    fs::write(&undo_file, &current).map_err(|e| e.to_string())?;

    let (updated, text) = match shortcut {
        Some((modifier, key)) => (
            with_shortcut(&current, group, position, modifier, key),
            shortcut_text(modifier, key),
        ),
        None => (with_default(&current, group, position), "back to normal".to_string()),
    };
    let updated = updated.ok_or("bad key report")?;
    isku.write_profile_report(profile, &group.report, &updated)?;
    let name = if group.report.id == EASYZONE_KEYS.report.id {
        format!("Shift+{}", group.labels[position])
    } else {
        group.labels[position].to_string()
    };
    println!("Profile {}: {name} is now {text}. Read back and confirmed.", profile + 1);
    println!("Before this change: {}", undo_file.display());
    Ok(())
}

fn restore(folder: &str, profile: &str) -> Result<(), String> {
    let profile = profile_index(profile).ok_or("use a profile number, 1 to 5")?;
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
    println!("Profile {} matches the backup. Reports restored: {restored}", profile + 1);
    Ok(())
}

fn reset(number: &str) -> Result<(), String> {
    let profile = profile_index(number).ok_or("use a profile number, 1 to 5")?;
    let isku = Isku::open()?;

    // Save the whole profile first, so `restore` can undo the reset.
    let folder = new_backup_folder()?;
    for report in &PROFILE_REPORTS {
        let bytes = isku.read_profile_report(profile, report)?;
        save(Some(&folder), &format!("profile-{}-{}.bin", profile + 1, report.name), &bytes)?;
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

    println!("Profile {}: {changed} groups reset. Thumb and M keys left as they were.", profile + 1);
    println!("Undo with: cargo run -- restore {} {}", folder.display(), profile + 1);
    Ok(())
}

fn activate(number: &str) -> Result<(), String> {
    let profile = profile_index(number).ok_or("use a profile number, 1 to 5")?;
    Isku::open()?.activate_profile(profile)?;
    println!("Active profile is now {}", profile + 1);
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] => show(None),
        ["backup"] => show(Some(&new_backup_folder()?)),
        ["set", profile, label, value] => set_key(profile, label, value),
        ["restore", folder, profile] => restore(folder, profile),
        ["activate", number] => activate(number),
        ["reset", number] => reset(number),
        _ => Err(
            "use: backup, set <1-5> T1 7, restore <folder> <1-5>, reset <1-5> or activate <1-5>".to_string(),
        ),
    }
}

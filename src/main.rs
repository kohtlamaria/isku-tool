//! cargo run                         show all 5 profiles
//! cargo run -- backup               show them and save them to backups/
//! cargo run -- set 2 T1 7           set a thumb key on profile 2 to a number
//! cargo run -- restore <folder> 2   put profile 2 back from a backup
//! cargo run -- activate <1-5>       switch the active profile

use isku_tool::device::Isku;
use isku_tool::protocol::{
    bindings, describe, describe_macro, digit_key, parse_macro, profile_index,
    valid_profile_report, with_shortcut, KeyGroup, ACTION_MACRO, EASYZONE_DEFAULTS,
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
        let unchanged = group.first_macro_index == EASYZONE_KEYS.first_macro_index
            && binding.action == EASYZONE_DEFAULTS[i];
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

fn set_thumb_key(profile: &str, label: &str, digit: &str) -> Result<(), String> {
    let profile = profile_index(profile).ok_or("use a profile number, 1 to 5")?;
    let position = THUMB_KEYS
        .labels
        .iter()
        .position(|l| l.eq_ignore_ascii_case(label))
        .ok_or("use T1, T2, T3, Shift+T1, Shift+T2 or Shift+T3")?;
    let mut digits = digit.chars();
    let key = match (digits.next(), digits.next()) {
        (Some(c), None) => digit_key(c),
        _ => None,
    }
    .ok_or("use a single number key, 0 to 9")?;
    if !has_backup(profile) {
        return Err(format!("no backup of profile {} found, run: cargo run -- backup", profile + 1));
    }

    let isku = Isku::open()?;
    let current = isku.read_profile_report(profile, &THUMB_KEYS.report)?;

    // Keep the exact bytes from just before this change, so it can be undone.
    let undo = PathBuf::from("backups").join("changes");
    fs::create_dir_all(&undo).map_err(|e| e.to_string())?;
    let undo_file = undo.join(format!(
        "{}-profile-{}-{}.bin",
        timestamp()?,
        profile + 1,
        THUMB_KEYS.report.name
    ));
    fs::write(&undo_file, &current).map_err(|e| e.to_string())?;

    let updated = with_shortcut(&current, &THUMB_KEYS, position, key).ok_or("bad key report")?;
    isku.write_profile_report(profile, &THUMB_KEYS.report, &updated)?;
    println!(
        "Profile {}: {} is now key {digit}. Read back and confirmed.",
        profile + 1,
        THUMB_KEYS.labels[position]
    );
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
        ["set", profile, label, digit] => set_thumb_key(profile, label, digit),
        ["restore", folder, profile] => restore(folder, profile),
        ["activate", number] => activate(number),
        _ => Err(
            "use: backup, set <1-5> T1 7, restore <folder> <1-5> or activate <1-5>".to_string(),
        ),
    }
}

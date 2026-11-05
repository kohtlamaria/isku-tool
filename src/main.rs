//! `cargo run` shows all 5 profiles. `cargo run -- backup` also saves them.
//! The keyboard's key settings are never changed by this program.

use isku_tool::device::Isku;
use isku_tool::protocol::{
    bindings, describe, describe_macro, parse_macro, KeyGroup, ACTION_MACRO, EASYZONE_DEFAULTS,
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
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let folder = PathBuf::from("backups").join(format!("backup-{seconds}"));
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    Ok(folder)
}

fn main() -> Result<(), String> {
    let backup = std::env::args().nth(1).as_deref() == Some("backup");
    let folder = if backup { Some(new_backup_folder()?) } else { None };
    let folder = folder.as_deref();

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

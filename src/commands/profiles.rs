//! Whole-profile commands: `restore`, `reset`, `copy` and `activate`.
//!
//! `reset`, `copy` and `restore` read everything they need before the first
//! write, and record every report they write as one change, so a single
//! `undo` puts the whole command back.

use super::{backup_file_name, profile_arg, require_backup, ChangeSet};
use isku_tool::device::Isku;
use isku_tool::protocol::{
    bindings, describe_light, for_profile, key_label, valid_profile_report, with_all_defaults,
    with_capslock, with_light, Report, ACTION_MACRO, CAPSLOCK_DEFAULT, EASYZONE_KEYS, FUNCTION_KEYS,
    KEYS_CAPSLOCK, LIGHT, LIGHT_DEFAULT, MACRO_KEYS, MEDIA_KEYS, PROFILE_REPORTS, THUMB_KEYS,
    write_order,
};
use std::fs;
use std::path::Path;

/// One planned write: the report, its bytes now, and the bytes to write.
struct Planned {
    report: Report,
    current: Vec<u8>,
    updated: Vec<u8>,
    message: String,
}

/// Writes every planned report that changes anything, as one change, in the
/// order of PROFILE_REPORTS, which `undo` reverses. Returns how many were
/// written.
fn write_all(isku: &Isku, profile: u8, label: &str, plan: &mut [Planned]) -> Result<usize, String> {
    plan.sort_by_key(|item| write_order(&item.report));
    let mut changes = ChangeSet::new(label);
    let mut written = 0;
    for item in plan.iter() {
        if item.updated != item.current {
            changes.write(isku, profile, &item.report, &item.current, &item.updated)?;
            println!("{}", item.message);
            written += 1;
        }
    }
    changes.print_undo_hint();
    Ok(written)
}

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
        backups.push((*report, bytes));
    }

    let isku = Isku::open()?;
    let mut plan = Vec::new();
    for (report, bytes) in backups {
        plan.push(Planned {
            report,
            current: isku.read_profile_report(profile, &report)?,
            updated: bytes,
            message: format!("Restored {}", report.name),
        });
    }
    let restored = write_all(&isku, profile, &format!("restore-{}", profile + 1), &mut plan)?;
    println!(
        "Profile {} matches the backup. Reports restored: {restored}",
        profile + 1
    );
    Ok(())
}

pub fn reset(number: &str) -> Result<(), String> {
    let profile = profile_arg(number)?;
    require_backup(profile)?;
    let isku = Isku::open()?;

    let mut plan = Vec::new();
    for group in [&EASYZONE_KEYS, &FUNCTION_KEYS, &MEDIA_KEYS] {
        let current = isku.read_profile_report(profile, &group.report)?;
        let updated = with_all_defaults(&current, group).ok_or("bad key report")?;
        plan.push(Planned {
            report: group.report,
            current,
            updated,
            message: format!("Reset {}", group.title),
        });
    }
    let current = isku.read_profile_report(profile, &KEYS_CAPSLOCK)?;
    let updated = with_capslock(&current, CAPSLOCK_DEFAULT).ok_or("bad Caps Lock report")?;
    plan.push(Planned {
        report: KEYS_CAPSLOCK,
        current,
        updated,
        message: "Reset Caps Lock to Easy-Shift".to_string(),
    });
    let current = isku.read_profile_report(profile, &LIGHT)?;
    let updated = with_light(&current, LIGHT_DEFAULT).ok_or("bad light report")?;
    plan.push(Planned {
        report: LIGHT,
        current,
        updated,
        message: format!("Reset lighting to {}", describe_light(LIGHT_DEFAULT)),
    });

    let changed = write_all(&isku, profile, &format!("reset-{}", profile + 1), &mut plan)?;
    println!(
        "Profile {}: {changed} groups reset. Thumb and M keys left as they were.",
        profile + 1
    );
    Ok(())
}

/// Copies every key and lighting setting from one profile to another. Macros
/// are separate reports that `copy` does not copy yet, so keys that play a
/// macro are listed with a warning.
pub fn copy(from: &str, to: &str) -> Result<(), String> {
    let from = profile_arg(from)?;
    let to = profile_arg(to)?;
    if from == to {
        return Err("copy needs two different profiles".into());
    }
    require_backup(to)?;
    let isku = Isku::open()?;

    // Read and check both profiles before writing anything, so a bad source
    // report or a failed read stops the copy before the target changes.
    let mut plan = Vec::new();
    let mut macro_keys = Vec::new();
    for report in &PROFILE_REPORTS {
        let source = isku.read_profile_report(from, report)?;
        let updated = for_profile(report, from, &source, to)
            .ok_or_else(|| format!("profile {} has a bad {} report", from + 1, report.name))?;
        for group in [&THUMB_KEYS, &MACRO_KEYS, &EASYZONE_KEYS] {
            if group.report.id == report.id {
                let keys = bindings(&source, group.labels.len()).ok_or("bad key report")?;
                for (position, binding) in keys.iter().enumerate() {
                    if binding.action == ACTION_MACRO {
                        macro_keys.push(key_label(group, position));
                    }
                }
            }
        }
        plan.push(Planned {
            report: *report,
            current: isku.read_profile_report(to, report)?,
            updated,
            message: format!("Copied {}", report.name),
        });
    }

    let label = format!("copy-{}-to-{}", from + 1, to + 1);
    let changed = write_all(&isku, to, &label, &mut plan)?;
    println!(
        "Profile {} now matches profile {}: {changed} reports written.",
        to + 1,
        from + 1
    );
    if !macro_keys.is_empty() {
        println!(
            "Macros are not copied yet. These keys play profile {}'s own macro in that slot: {}",
            to + 1,
            macro_keys.join(", ")
        );
    }
    Ok(())
}

pub fn activate(number: &str) -> Result<(), String> {
    let profile = profile_arg(number)?;
    Isku::open()?.activate_profile(profile)?;
    println!("Active profile is now {}", profile + 1);
    Ok(())
}

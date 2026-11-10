//! Whole-profile commands: `restore`, `reset`, `copy` and `activate`.
//!
//! `reset`, `copy` and `restore` read everything they need before the first
//! write, and record every report and macro they write as one change, so a
//! single `undo` puts the whole command back.

use super::{backup_file_name, check_undoable_macro, profile_arg, require_backup, ChangeSet};
use isku_tool::device::Isku;
use isku_tool::protocol::{
    bindings, describe_light, empty_macro_report, for_profile, macro_for_profile, macro_slot,
    parse_macro_file_name, slot_key_label, valid_macro_report, valid_profile_report,
    with_all_defaults, with_capslock, with_light, write_order, Report, ACTION_MACRO,
    CAPSLOCK_DEFAULT, EASYZONE_KEYS, FUNCTION_KEYS, KEYS_CAPSLOCK, LIGHT, LIGHT_DEFAULT,
    MACRO_KEYS, MEDIA_KEYS, PROFILE_REPORTS, THUMB_KEYS,
};
use std::fs;
use std::path::Path;

/// One planned write: the report, its bytes now, and the bytes to write.
pub(super) struct Planned {
    pub(super) report: Report,
    pub(super) current: Vec<u8>,
    pub(super) updated: Vec<u8>,
    pub(super) message: String,
}

/// One planned macro write: the slot, the macro in it now (`None` when it
/// holds none), and the macro to write.
pub(super) struct PlannedMacro {
    pub(super) key_index: u8,
    pub(super) current: Option<Vec<u8>>,
    pub(super) updated: Vec<u8>,
    pub(super) message: String,
}

/// Writes every planned macro and report that changes anything, as one
/// change. Macros go first, so a key never plays a half-written slot, then
/// reports in the order of PROFILE_REPORTS. `undo` goes the other way.
/// Returns how many reports and macros were written.
pub(super) fn write_all(
    isku: &Isku,
    profile: u8,
    label: &str,
    macros: &[PlannedMacro],
    plan: &mut [Planned],
) -> Result<usize, String> {
    plan.sort_by_key(|item| write_order(&item.report));
    let mut changes = ChangeSet::new(label);
    let mut written = 0;
    for item in macros {
        if item.current.as_deref() != Some(item.updated.as_slice()) {
            changes.write_macro(
                isku,
                profile,
                item.key_index,
                item.current.as_deref(),
                &item.updated,
            )?;
            println!("{}", item.message);
            written += 1;
        }
    }
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

/// The name of the key that plays a slot, for messages.
pub(super) fn slot_name(key_index: u8) -> String {
    slot_key_label(key_index).unwrap_or_else(|| format!("slot 0x{key_index:02x}"))
}

/// The macro in a slot now, refused while planning if `undo` could not put
/// it back, so nothing is written before the problem shows.
pub(super) fn current_macro(
    isku: &Isku,
    profile: u8,
    key_index: u8,
) -> Result<Option<Vec<u8>>, String> {
    let current = isku.read_macro(profile, key_index)?;
    check_undoable_macro(profile, key_index, current.as_deref())?;
    Ok(current)
}

/// The macro slots the keys in one profile report play.
pub(super) fn played_slots(report: &Report, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut slots = Vec::new();
    for group in [&THUMB_KEYS, &MACRO_KEYS, &EASYZONE_KEYS] {
        if group.report.id != report.id {
            continue;
        }
        let keys = bindings(bytes, group.labels.len()).ok_or("bad key report")?;
        for (position, binding) in keys.iter().enumerate() {
            if binding.action == ACTION_MACRO {
                slots.extend(macro_slot(group, position));
            }
        }
    }
    Ok(slots)
}

/// The macro to write over a slot whose source holds none, so the key plays
/// nothing, or None when the slot holds none either.
fn nothing_for(
    profile: u8,
    key_index: u8,
    current: Option<&[u8]>,
) -> Result<Option<Vec<u8>>, String> {
    if current.is_none() {
        return Ok(None);
    }
    empty_macro_report(profile, key_index)
        .map(Some)
        .ok_or_else(|| format!("slot 0x{key_index:02x} cannot hold a macro"))
}

/// What a backup folder holds about one profile's macros: the macros it
/// saved, checked, and the slots it noted as empty (`.empty`). `backup`
/// saves both for every key that plays a macro. Older backups have no
/// `.empty` notes, and the ones from `reset` and `copy` no macros at all.
struct BackupMacros {
    saved: Vec<(u8, Vec<u8>)>,
    empty: Vec<u8>,
}

fn backup_macros(folder: &Path, profile: u8) -> Result<BackupMacros, String> {
    let mut found = BackupMacros {
        saved: Vec::new(),
        empty: Vec::new(),
    };
    let entries = fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", folder.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((file_profile, key_index, empty)) = parse_macro_file_name(&name) else {
            continue;
        };
        if file_profile != profile {
            continue;
        }
        if empty {
            found.empty.push(key_index);
            continue;
        }
        let path = entry.path();
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if !valid_macro_report(profile, key_index, &bytes) {
            return Err(format!("{} is not a valid backup", path.display()));
        }
        found.saved.push((key_index, bytes));
    }
    found.saved.sort_by_key(|(key_index, _)| *key_index);
    found.empty.sort_unstable();
    Ok(found)
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
    let BackupMacros { saved, empty } = backup_macros(folder, profile)?;
    let mut played = Vec::new();
    for (report, bytes) in &backups {
        played.extend(played_slots(report, bytes)?);
    }
    // Only slots the backup's keys play are touched, and a saved macro wins
    // over an empty note for the same slot.
    let saved: Vec<(u8, Vec<u8>)> = saved
        .into_iter()
        .filter(|(key_index, _)| played.contains(key_index))
        .collect();
    let has_saved = |key_index: u8| saved.iter().any(|(s, _)| *s == key_index);
    let empty: Vec<u8> = empty
        .into_iter()
        .filter(|key_index| played.contains(key_index) && !has_saved(*key_index))
        .collect();
    // Played slots with neither a macro nor an empty note: an older or
    // interrupted backup. Those are left alone.
    let missing: Vec<u8> = played
        .iter()
        .copied()
        .filter(|key_index| !has_saved(*key_index) && !empty.contains(key_index))
        .collect();

    let isku = Isku::open()?;
    let mut macros = Vec::new();
    for (key_index, bytes) in saved {
        macros.push(PlannedMacro {
            key_index,
            current: current_macro(&isku, profile, key_index)?,
            updated: bytes,
            message: format!("Restored the macro for {}", slot_name(key_index)),
        });
    }
    for key_index in empty {
        let current = current_macro(&isku, profile, key_index)?;
        if let Some(updated) = nothing_for(profile, key_index, current.as_deref())? {
            macros.push(PlannedMacro {
                key_index,
                current,
                updated,
                message: format!(
                    "Cleared the macro for {}, which held none in the backup",
                    slot_name(key_index)
                ),
            });
        }
    }
    let mut plan = Vec::new();
    for (report, bytes) in backups {
        plan.push(Planned {
            report,
            current: isku.read_profile_report(profile, &report)?,
            updated: bytes,
            message: format!("Restored {}", report.name),
        });
    }
    let label = format!("restore-{}", profile + 1);
    let restored = write_all(&isku, profile, &label, &macros, &mut plan)?;
    if missing.is_empty() {
        println!(
            "Profile {} matches the backup. Reports and macros restored: {restored}",
            profile + 1
        );
    } else {
        let keys: Vec<String> = missing.iter().map(|&k| slot_name(k)).collect();
        println!(
            "Profile {}'s keys and lighting match the backup. Reports and macros restored: \
             {restored}. This backup does not hold the macros for {}, so they were left as \
             they are.",
            profile + 1,
            keys.join(", ")
        );
    }
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

    let label = format!("reset-{}", profile + 1);
    let changed = write_all(&isku, profile, &label, &[], &mut plan)?;
    println!(
        "Profile {}: {changed} groups reset. Thumb and M keys left as they were.",
        profile + 1
    );
    Ok(())
}

/// Copies every key, lighting setting and macro from one profile to
/// another. Each key that plays a macro gets the source's macro in the same
/// slot. A source slot with no macro becomes one that plays nothing, so the
/// key does nothing on both profiles. Slots no key plays are left alone.
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
    let mut macros = Vec::new();
    for report in &PROFILE_REPORTS {
        let source = isku.read_profile_report(from, report)?;
        let updated = for_profile(report, from, &source, to)
            .ok_or_else(|| format!("profile {} has a bad {} report", from + 1, report.name))?;
        for key_index in played_slots(report, &source)? {
            let current = current_macro(&isku, to, key_index)?;
            let (updated, message) = match isku.read_macro(from, key_index)? {
                Some(bytes) => (
                    macro_for_profile(from, key_index, &bytes, to).ok_or_else(|| {
                        format!(
                            "profile {} has a bad macro for {}",
                            from + 1,
                            slot_name(key_index)
                        )
                    })?,
                    format!("Copied the macro for {}", slot_name(key_index)),
                ),
                None => match nothing_for(to, key_index, current.as_deref())? {
                    Some(nothing) => (
                        nothing,
                        format!(
                            "Cleared the macro for {}, which holds none on profile {}",
                            slot_name(key_index),
                            from + 1
                        ),
                    ),
                    // Both slots empty: the key already does nothing.
                    None => continue,
                },
            };
            macros.push(PlannedMacro {
                key_index,
                current,
                updated,
                message,
            });
        }
        plan.push(Planned {
            report: *report,
            current: isku.read_profile_report(to, report)?,
            updated,
            message: format!("Copied {}", report.name),
        });
    }

    let label = format!("copy-{}-to-{}", from + 1, to + 1);
    let changed = write_all(&isku, to, &label, &macros, &mut plan)?;
    println!(
        "Profile {} now matches profile {}: {changed} reports and macros written.",
        to + 1,
        from + 1
    );
    Ok(())
}

pub fn activate(number: &str) -> Result<(), String> {
    let profile = profile_arg(number)?;
    Isku::open()?.activate_profile(profile)?;
    println!("Active profile is now {}", profile + 1);
    Ok(())
}

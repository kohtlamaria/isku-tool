//! `export` and `import`: one profile as a text file that can be read and
//! edited. The format is in `protocol::profile_text`.

use super::profiles::{current_macro, played_slots, write_all, Planned, PlannedMacro};
use super::{profile_arg, require_backup};
use isku_tool::device::Isku;
use isku_tool::protocol::{
    apply_settings, key_label, macro_from_text, macro_slot, parse_macro, parse_profile_text,
    plays_once, profile_text, KeyValue, Report, Setting, KEYS_CAPSLOCK, LEFT_OUT_NOTE, LIGHT,
    PROFILE_TEXT_GROUPS,
};
use std::fs;
use std::io::Write;

pub fn export(profile: &str, file: &str) -> Result<(), String> {
    let profile = profile_arg(profile)?;
    let isku = Isku::open()?;
    let mut reports = Vec::new();
    let mut macros = Vec::new();
    for report in PROFILE_TEXT_GROUPS
        .iter()
        .map(|group| group.report)
        .chain([KEYS_CAPSLOCK, LIGHT])
    {
        let bytes = isku.read_profile_report(profile, &report)?;
        for key_index in played_slots(&report, &bytes)? {
            match isku.read_macro(profile, key_index)? {
                // A macro that does not parse is left out of `macros`, so
                // the file says it could not be read.
                Some(bytes) => macros.extend(parse_macro(&bytes).map(|m| (key_index, Some(m)))),
                None => macros.push((key_index, None)),
            }
        }
        reports.push((report, bytes));
    }
    let text = profile_text(profile, &reports, &macros)?;

    // Never overwrite a file, which could be an earlier export or anything
    // else, and never leave half a file behind: a failed write removes it.
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)
        .map_err(|e| format!("{file}: {e}"))?;
    if let Err(e) = out.write_all(text.as_bytes()).and_then(|()| out.sync_all()) {
        drop(out);
        let _ = fs::remove_file(file);
        return Err(format!("{file}: {e}"));
    }
    let left_out = text.lines().filter(|l| l.ends_with(LEFT_OUT_NOTE)).count();
    println!("Saved profile {} to {file}.", profile + 1);
    if left_out > 0 {
        println!(
            "Left out, as # lines, because import cannot set them: {left_out} of the settings."
        );
    }
    Ok(())
}

pub fn import(file: &str, profile: &str) -> Result<(), String> {
    let profile = profile_arg(profile)?;
    let text = fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let settings = parse_profile_text(&text).map_err(|e| format!("{file}, {e}"))?;
    if settings.is_empty() {
        return Err(format!("{file} sets nothing"));
    }
    require_backup(profile)?;
    let isku = Isku::open()?;

    // Read everything the file touches before the first write.
    let mut needed: Vec<Report> = Vec::new();
    for setting in &settings {
        let report = match setting {
            Setting::Key { group, .. } => group.report,
            Setting::CapsLock(_) => KEYS_CAPSLOCK,
            Setting::Light(_) => LIGHT,
        };
        if !needed.iter().any(|r| r.id == report.id) {
            needed.push(report);
        }
    }
    let mut reports = Vec::new();
    for report in needed {
        reports.push((report, isku.read_profile_report(profile, &report)?));
    }

    let mut macros = Vec::new();
    for setting in &settings {
        let Setting::Key {
            group,
            position,
            value: KeyValue::Macro(text, keystrokes),
        } = setting
        else {
            continue;
        };
        let label = key_label(group, *position);
        // parse_profile_text already refused macros on keys without a slot.
        let key_index =
            macro_slot(group, *position).ok_or_else(|| format!("{label} cannot play a macro"))?;
        let current = current_macro(&isku, profile, key_index)?;
        let old = current.as_deref().and_then(parse_macro);
        // The same keystrokes, played once: only the name could differ, so
        // the slot is left as it is.
        if old
            .as_ref()
            .is_some_and(|m| m.keystrokes == *keystrokes && plays_once(m))
        {
            continue;
        }
        let updated = macro_from_text(profile, key_index, old.as_ref(), text, keystrokes.clone())
            .ok_or_else(|| format!("the macro for {label} does not fit"))?;
        macros.push(PlannedMacro {
            key_index,
            current,
            updated,
            message: format!("Wrote the macro for {label}"),
        });
    }

    let current = reports.clone();
    apply_settings(&mut reports, &settings)?;
    let mut plan: Vec<Planned> = reports
        .into_iter()
        .zip(current)
        .map(|((report, updated), (_, current))| Planned {
            report,
            current,
            updated,
            message: format!("Wrote {}", report.name),
        })
        .collect();
    let label = format!("import-{}", profile + 1);
    let written = write_all(&isku, profile, &label, &macros, &mut plan)?;
    if written == 0 {
        println!("Profile {} already matches {file}.", profile + 1);
    } else {
        println!(
            "Imported {file} to profile {}: {written} reports and macros written, each read \
             back and confirmed.",
            profile + 1
        );
    }
    Ok(())
}

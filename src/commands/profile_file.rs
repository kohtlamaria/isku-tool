//! `export` and `import`: one profile as a text file that can be read and
//! edited. The format is in `protocol::profile_text`.

use super::profiles::{current_macro, played_slots, write_all, Planned, PlannedMacro};
use super::show::print_warnings;
use super::{profile_arg, require_backup};
use isku_tool::device::Isku;
use isku_tool::protocol::{
    apply_settings, changed_values, key_label, macro_from_text, macro_slot, parse_macro,
    parse_profile_text, plays_once, profile_text, risky_keys, KeyValue, Macro, Report, Setting,
    KEYS_CAPSLOCK, LEFT_OUT_NOTE, LIGHT, PROFILE_TEXT_GROUPS,
};
use std::fs;
use std::io::Write;

/// The reports a profile file lists, in order.
pub(super) fn profile_text_reports() -> impl Iterator<Item = Report> {
    PROFILE_TEXT_GROUPS
        .iter()
        .map(|group| group.report)
        .chain([KEYS_CAPSLOCK, LIGHT])
}

/// Reads what `profile_text` needs from the keyboard: the reports, and the
/// macro in each slot a key plays. A macro that does not parse is left out,
/// so the text says it could not be read.
pub(super) fn read_profile(isku: &Isku, profile: u8) -> Result<ProfileData, String> {
    let mut reports = Vec::new();
    let mut macros = Vec::new();
    for report in profile_text_reports() {
        let bytes = isku.read_profile_report(profile, &report)?;
        for key_index in played_slots(&report, &bytes)? {
            match isku.read_macro(profile, key_index)? {
                Some(bytes) => macros.extend(parse_macro(&bytes).map(|m| (key_index, Some(m)))),
                None => macros.push((key_index, None)),
            }
        }
        reports.push((report, bytes));
    }
    Ok((reports, macros))
}

/// A profile's reports and its macros, as `profile_text` takes them.
pub(super) type ProfileData = (Vec<(Report, Vec<u8>)>, Vec<(u8, Option<Macro>)>);

pub fn export(profile: &str, file: &str) -> Result<(), String> {
    let profile = profile_arg(profile)?;
    let isku = Isku::open()?;
    let (reports, macros) = read_profile(&isku, profile)?;
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
    let warnings: Vec<String> = risky_keys(&reports)?
        .into_iter()
        .map(|key| format!("Profile {}: {key}", profile + 1))
        .collect();
    print_warnings(&warnings);
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

    // Read the whole profile before the first write, so the changes can be
    // listed from its text before and after.
    let (mut reports, macros_before) = read_profile(&isku, profile)?;

    let mut macros = Vec::new();
    // The macro each macro line leaves in its slot, for the text after.
    let mut macros_after = macros_before.clone();
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
        macros_after.retain(|(slot, _)| *slot != key_index);
        if let Some(same) = old
            .as_ref()
            .filter(|m| m.keystrokes == *keystrokes && plays_once(m))
        {
            macros_after.push((key_index, Some(same.clone())));
            continue;
        }
        let updated = macro_from_text(profile, key_index, old.as_ref(), text, keystrokes.clone())
            .ok_or_else(|| format!("the macro for {label} does not fit"))?;
        macros_after.extend(parse_macro(&updated).map(|m| (key_index, Some(m))));
        macros.push(PlannedMacro {
            key_index,
            current,
            updated,
            message: format!("Wrote the macro for {label}"),
        });
    }

    let current = reports.clone();
    apply_settings(&mut reports, &settings)?;

    // Only for showing, so a report it cannot describe never stops the write.
    let changes = match (
        profile_text(profile, &current, &macros_before),
        profile_text(profile, &reports, &macros_after),
    ) {
        (Ok(before), Ok(after)) => changed_values(&before, &after),
        _ => {
            println!("Could not list the changes, writing them anyway.");
            Vec::new()
        }
    };
    if !changes.is_empty() {
        println!("Changes to profile {}:", profile + 1);
        let width = changes
            .iter()
            .map(|(label, ..)| label.len())
            .max()
            .unwrap_or(0);
        for (label, old, new) in &changes {
            println!("  {label:width$}  {old}  ->  {new}");
        }
    }

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

//! `cargo run` and `cargo run -- backup`: list all profiles, optionally saving them.

use super::{backup_file_name, save};
use isku_tool::device::Isku;
use isku_tool::protocol::{
    bindings, capslock_of, describe, describe_capslock, describe_light, describe_macro,
    empty_macro_file_name, macro_file_name, macro_slot, parse_light, parse_macro, KeyGroup,
    ACTION_MACRO, EASYZONE_KEYS, FUNCTION_KEYS, INFO, KEYS_CAPSLOCK, LAST_SET, LIGHT, MACRO_KEYS,
    MEDIA_KEYS, PROFILE_COUNT, PROFILE_REPORTS, THUMB_KEYS,
};
use std::path::Path;
use std::time::Instant;

fn show_group(
    isku: &Isku,
    profile: u8,
    group: &KeyGroup,
    report: &[u8],
    only_changed: bool,
    folder: Option<&Path>,
) -> Result<(), String> {
    let keys = bindings(report, group.labels.len()).ok_or("key report too short")?;
    println!("  {}", group.title);
    let mut shown = 0;
    for (i, (label, binding)) in group.labels.iter().zip(&keys).enumerate() {
        let mut detail = describe(*binding);
        if binding.action == ACTION_MACRO {
            // A key can point at a macro slot that holds nothing, for example
            // after the old software or an older `copy`. Any other read
            // error still stops the listing.
            match macro_slot(group, i) {
                None => detail = "macro, but this key has no macro slot".to_string(),
                Some(key_index) => match isku.read_macro(profile, key_index)? {
                    Some(bytes) => {
                        save(folder, &macro_file_name(profile, key_index), &bytes)?;
                        if let Some(value) = parse_macro(&bytes) {
                            detail = format!("macro {}", describe_macro(&value));
                        }
                    }
                    None => {
                        // Noted, so `restore` knows the slot was empty
                        // rather than missing from the backup.
                        save(folder, &empty_macro_file_name(profile, key_index), &[])?;
                        detail = "macro, but none is saved in this slot".to_string();
                    }
                },
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
    Ok(())
}

pub fn show(folder: Option<&Path>) -> Result<(), String> {
    let started = Instant::now();
    let isku = Isku::open()?;
    let active_before = isku.active_profile()?;

    for profile in 0..PROFILE_COUNT {
        let marker = if profile == active_before {
            " (active)"
        } else {
            ""
        };
        println!("Profile {}{marker}", profile + 1);

        // Read each report once, then use it for both the listing and the backup.
        let mut reports = Vec::new();
        for report in &PROFILE_REPORTS {
            let bytes = isku.read_profile_report(profile, report)?;
            save(folder, &backup_file_name(profile, report), &bytes)?;
            reports.push((report.id, bytes));
        }
        let bytes_of = |id: u8| {
            reports
                .iter()
                .find(|(r, _)| *r == id)
                .map(|(_, b)| b.as_slice())
                .ok_or("missing report")
        };

        for (group, only_changed) in [
            (&THUMB_KEYS, false),
            (&MACRO_KEYS, false),
            (&EASYZONE_KEYS, true),
            (&FUNCTION_KEYS, true),
            (&MEDIA_KEYS, true),
        ] {
            show_group(
                &isku,
                profile,
                group,
                bytes_of(group.report.id)?,
                only_changed,
                folder,
            )?;
        }
        if let Some(action) = capslock_of(bytes_of(KEYS_CAPSLOCK.id)?) {
            println!("  Caps Lock: {}", describe_capslock(action));
        }
        if let Some(light) = parse_light(bytes_of(LIGHT.id)?) {
            println!("  Lighting: {}", describe_light(light));
        }
    }

    for report in [&INFO, &LAST_SET] {
        save(
            folder,
            &format!("{}.bin", report.name),
            &isku.read_global(report)?,
        )?;
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
    println!(
        "Took {:.1} s: {}",
        started.elapsed().as_secs_f64(),
        isku.timing()
    );
    Ok(())
}

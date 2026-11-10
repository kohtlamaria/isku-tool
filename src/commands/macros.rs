//! `macro`: write a macro into a key's slot and make the key play it.

use super::{profile_arg, require_backup, ChangeSet};
use isku_tool::device::Isku;
use isku_tool::protocol::{
    find_key, key_label, macro_from_text, macro_slot, parse_macro, parse_macro_text, with_macro,
};

pub fn set_macro(profile: &str, label: &str, text: &str) -> Result<(), String> {
    let profile = profile_arg(profile)?;
    let (group, position, key_index) = find_key(label)
        .and_then(|(group, position)| Some((group, position, macro_slot(group, position)?)))
        .ok_or(
            "macros work on T1-T3, M1-M5, and with Easy-Shift: Shift+T1, Shift+M1, Shift+Q \
             and the rest of the zone",
        )?;
    let keystrokes = parse_macro_text(text).ok_or(
        "write the macro as keys or shortcuts separated by commas, such as \
         \"Ctrl+C, wait 50, Ctrl+V\". wait is in milliseconds.",
    )?;
    require_backup(profile)?;

    let isku = Isku::open()?;
    let current_keys = isku.read_profile_report(profile, &group.report)?;
    let current_macro = isku.read_macro(profile, key_index)?;
    let old = current_macro.as_deref().and_then(parse_macro);
    let updated_macro = macro_from_text(profile, key_index, old.as_ref(), text, keystrokes)
        .ok_or("macro does not fit")?;
    let updated_keys = with_macro(&current_keys, group, position).ok_or("bad key report")?;

    let name = key_label(group, position);
    let macro_same = current_macro.as_deref() == Some(updated_macro.as_slice());
    if macro_same && updated_keys == current_keys {
        println!("Profile {}: {name} already plays {text}.", profile + 1);
        return Ok(());
    }

    // The macro goes in first, so the key never plays a half-written slot.
    let mut changes = ChangeSet::new(&format!("macro-{}", profile + 1));
    if !macro_same {
        changes.write_macro(
            &isku,
            profile,
            key_index,
            current_macro.as_deref(),
            &updated_macro,
        )?;
    }
    if updated_keys != current_keys {
        changes.write(&isku, profile, &group.report, &current_keys, &updated_keys)?;
    }
    println!(
        "Profile {}: {name} now plays {text}. Read back and confirmed.",
        profile + 1
    );
    changes.print_undo_hint();
    Ok(())
}

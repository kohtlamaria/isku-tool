//! `set`: change one key on one profile.

use super::{profile_arg, require_backup, ChangeSet};
use isku_tool::device::Isku;
use isku_tool::protocol::{
    action_code, capslock_action, capslock_of, describe_capslock, find_key, is_easyshift,
    key_label, parse_shortcut, shortcut_text, with_action, with_capslock, with_default,
    with_shortcut, KEYS_CAPSLOCK,
};

pub fn set_key(profile: &str, label: &str, value: &str) -> Result<(), String> {
    let profile = profile_arg(profile)?;
    if label.eq_ignore_ascii_case("CapsLock") {
        return set_capslock(profile, value);
    }
    let (group, position) = find_key(label).ok_or(
        "use T1-T3, M1-M5, F1-F12, Mute, VolDown, VolUp, Play, Prev, Next, WWW, Computer, \
         CapsLock, or with Easy-Shift: Shift+T1, Shift+M1, Shift+Q and the rest of the zone",
    )?;
    let reset = value.eq_ignore_ascii_case("default");
    if reset && group.defaults.is_none() {
        return Err(
            "default only works for zone, F- and media keys, use restore for the others".into(),
        );
    }
    let action = action_code(value);
    let shortcut = if reset || action.is_some() {
        None
    } else {
        Some(parse_shortcut(value).ok_or(
            "use a key like 7, Q, F5, Space or Num1, optionally with Ctrl+, Shift+, Alt+ or Win+, \
             or an action: off, profile-up, profile-down, profile-cycle, profile-1 to profile-5, \
             easyshift",
        )?)
    };
    require_backup(profile)?;

    let isku = Isku::open()?;
    let current = isku.read_profile_report(profile, &group.report)?;

    let (updated, text) = match (shortcut, action) {
        (Some((modifier, key)), _) => (
            with_shortcut(&current, group, position, modifier, key),
            shortcut_text(modifier, key),
        ),
        (None, Some(code)) => (
            with_action(&current, group, position, code),
            value.to_ascii_lowercase(),
        ),
        (None, None) => (
            with_default(&current, group, position),
            "back to normal".to_string(),
        ),
    };
    let updated = updated.ok_or("bad key report")?;
    let name = key_label(group, position);
    if updated == current {
        println!("Profile {}: {name} is already {text}.", profile + 1);
        return Ok(());
    }

    let mut changes = ChangeSet::new(&format!("set-{}", profile + 1));
    changes.write(&isku, profile, &group.report, &current, &updated)?;
    println!(
        "Profile {}: {name} is now {text}. Read back and confirmed.",
        profile + 1
    );
    changes.print_undo_hint();
    Ok(())
}

/// Caps Lock has its own one-byte report, so it only takes a few actions.
fn set_capslock(profile: u8, value: &str) -> Result<(), String> {
    let action =
        capslock_action(value).ok_or("for CapsLock use easyshift, capslock, off or default")?;
    require_backup(profile)?;

    let isku = Isku::open()?;
    let current = isku.read_profile_report(profile, &KEYS_CAPSLOCK)?;
    let stored = capslock_of(&current).ok_or("bad Caps Lock report")?;
    let updated = with_capslock(&current, action).ok_or("bad Caps Lock report")?;
    // Both Easy-Shift codes do the same, so either one counts as already set.
    if updated == current || (is_easyshift(action) && is_easyshift(stored)) {
        println!(
            "Profile {}: Caps Lock is already {}.",
            profile + 1,
            describe_capslock(stored)
        );
        return Ok(());
    }
    let text = describe_capslock(action);

    let mut changes = ChangeSet::new(&format!("set-{}", profile + 1));
    changes.write(&isku, profile, &KEYS_CAPSLOCK, &current, &updated)?;
    println!(
        "Profile {}: Caps Lock is now {text}. Read back and confirmed.",
        profile + 1
    );
    changes.print_undo_hint();
    Ok(())
}

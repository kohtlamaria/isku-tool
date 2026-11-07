//! `set`: change one key on one profile.

use super::{profile_arg, require_backup, save_change};
use isku_tool::device::Isku;
use isku_tool::protocol::{
    action_code, find_key, parse_shortcut, shortcut_text, with_action, with_default, with_shortcut,
    EASYZONE_KEYS,
};

pub fn set_key(profile: &str, label: &str, value: &str) -> Result<(), String> {
    let profile = profile_arg(profile)?;
    let (group, position) = find_key(label).ok_or(
        "use T1-T3, M1-M5, F1-F12, Mute, VolDown, VolUp, Play, Prev, Next, WWW, Computer, \
         or with Easy-Shift: Shift+T1, Shift+M1, Shift+Q and the rest of the zone",
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
    let name = if group.report.id == EASYZONE_KEYS.report.id {
        format!("Shift+{}", group.labels[position])
    } else {
        group.labels[position].to_string()
    };
    if updated == current {
        println!("Profile {}: {name} is already {text}.", profile + 1);
        return Ok(());
    }

    let undo_file = save_change(profile, &group.report, &current)?;
    isku.write_profile_report(profile, &group.report, &updated)?;
    println!(
        "Profile {}: {name} is now {text}. Read back and confirmed.",
        profile + 1
    );
    println!("Before this change: {}", undo_file.display());
    Ok(())
}

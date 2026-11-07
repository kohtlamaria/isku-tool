//! `light`: brightness, dimming and timeout for one profile.

use super::{profile_arg, require_backup, save_change};
use isku_tool::device::Isku;
use isku_tool::protocol::{describe_light, parse_light, with_light, Light, LIGHT, LIGHT_DEFAULT};

pub fn set_light(number: &str, setting: &str, value: Option<&str>) -> Result<(), String> {
    let profile = profile_arg(number)?;
    require_backup(profile)?;
    let isku = Isku::open()?;
    let current = isku.read_profile_report(profile, &LIGHT)?;
    let light = parse_light(&current).ok_or("bad light report")?;

    let number = |text: Option<&str>| -> Result<u8, String> {
        let text = text.ok_or("use a whole number")?;
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
            return Err("use a whole number".into());
        }
        text.parse().map_err(|_| {
            "brightness and dim go from 0 to 5, timeout from 1 to 255 minutes".to_string()
        })
    };
    let wanted = match (setting.to_ascii_lowercase().as_str(), value) {
        ("default", None) => LIGHT_DEFAULT,
        ("brightness", _) => Light {
            brightness: number(value)?,
            ..light
        },
        ("dim", _) => Light {
            dimness: number(value)?,
            ..light
        },
        ("timeout", _) => Light {
            timeout: number(value)?,
            ..light
        },
        _ => {
            return Err("use: light <1-5> brightness|dim <0-5>, timeout <1-255> or default".into())
        }
    };
    let updated = with_light(&current, wanted)
        .ok_or("brightness and dim go from 0 to 5, timeout from 1 to 255 minutes")?;
    if updated == current {
        println!(
            "Profile {}: lighting already {}",
            profile + 1,
            describe_light(light)
        );
        return Ok(());
    }

    let undo_file = save_change(profile, &LIGHT, &current)?;

    isku.write_profile_report(profile, &LIGHT, &updated)?;
    println!(
        "Profile {}: lighting is now {}. Read back and confirmed.",
        profile + 1,
        describe_light(wanted)
    );
    println!("Before this change: {}", undo_file.display());
    Ok(())
}

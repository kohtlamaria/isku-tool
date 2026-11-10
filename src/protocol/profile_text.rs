//! The text form of a profile that `export` writes and `import` reads.
//!
//! Each line sets one thing, with the values `set`, `macro` and `light`
//! take:
//!
//! ```text
//! T1       = Ctrl+F5
//! T2       = profile-cycle
//! M1       = macro "Ctrl+C, wait 50, Ctrl+V"
//! Shift+Q  = default
//! CapsLock = easyshift
//! light    = brightness 3, dim 1, timeout 15
//! ```
//!
//! Blank lines and lines starting with `#` are skipped. `import` changes
//! only what a file lists, and leaves a key alone when it already does what
//! its line says, so importing a fresh export changes nothing.

use super::*;

/// What one key is set to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyValue {
    /// What the key does unchanged, for zone, F- and media keys.
    Default,
    /// A special action from ACTIONS.
    Action(u8),
    /// A shortcut: the modifier mask and the key code.
    Shortcut(u8, u8),
    /// A macro: its text and the keystrokes read from it.
    Macro(String, Vec<Keystroke>),
}

/// A lighting line: the values it sets. `default` sets all three.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LightChange {
    pub brightness: Option<u8>,
    pub dimness: Option<u8>,
    pub timeout: Option<u8>,
}

impl LightChange {
    pub fn apply(&self, light: Light) -> Light {
        Light {
            brightness: self.brightness.unwrap_or(light.brightness),
            dimness: self.dimness.unwrap_or(light.dimness),
            timeout: self.timeout.unwrap_or(light.timeout),
        }
    }
}

/// One line of a profile file.
#[derive(Clone, Debug)]
pub enum Setting {
    Key {
        group: &'static KeyGroup,
        position: usize,
        value: KeyValue,
    },
    CapsLock(u8),
    Light(LightChange),
}

/// The text after `macro`, without quotes, or None when the value is not a
/// macro.
fn macro_value(value: &str) -> Option<&str> {
    let (word, rest) = value.split_at_checked(5)?;
    if !word.eq_ignore_ascii_case("macro") || !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim();
    Some(
        rest.strip_prefix('"')
            .and_then(|r| r.strip_suffix('"'))
            .unwrap_or(rest),
    )
}

/// Reads a key's value: `default`, an action name, a shortcut, or `macro`
/// followed by macro text, in quotes or not.
pub fn parse_key_value(value: &str) -> Option<KeyValue> {
    if value.eq_ignore_ascii_case("default") {
        return Some(KeyValue::Default);
    }
    if let Some(code) = action_code(value) {
        return Some(KeyValue::Action(code));
    }
    if let Some(text) = macro_value(value) {
        let keystrokes = parse_macro_text(text)?;
        return Some(KeyValue::Macro(text.to_string(), keystrokes));
    }
    let (modifier, key) = parse_shortcut(value)?;
    Some(KeyValue::Shortcut(modifier, key))
}

/// Reads a lighting value: `default`, or any of `brightness N`, `dim N`
/// and `timeout N`, separated by commas.
pub fn parse_light_value(value: &str) -> Option<LightChange> {
    if value.eq_ignore_ascii_case("default") {
        return Some(LightChange {
            brightness: Some(LIGHT_DEFAULT.brightness),
            dimness: Some(LIGHT_DEFAULT.dimness),
            timeout: Some(LIGHT_DEFAULT.timeout),
        });
    }
    let mut change = LightChange::default();
    for part in value.split(',') {
        let (name, number) = part.trim().split_once(char::is_whitespace)?;
        let number = number.trim();
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let field = match name.to_ascii_lowercase().as_str() {
            "brightness" => &mut change.brightness,
            "dim" => &mut change.dimness,
            "timeout" => &mut change.timeout,
            _ => return None,
        };
        if field.is_some() {
            return None;
        }
        *field = Some(number.parse().ok()?);
    }
    // The defaults are valid, so this only fails on a value in the line.
    change.apply(LIGHT_DEFAULT).is_valid().then_some(change)
}

/// Reads a whole profile file. Every line is checked before anything is
/// returned, and an error names the line, so a bad file writes nothing.
pub fn parse_profile_text(text: &str) -> Result<Vec<Setting>, String> {
    let mut settings = Vec::new();
    // A file saved by Notepad can start with a byte order mark.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let at = |e: &str| format!("line {}: {e}", i + 1);
        let (label, value) = line
            .split_once('=')
            .ok_or_else(|| at("write it as key = value, such as T1 = Ctrl+F5"))?;
        let (label, value) = (label.trim(), value.trim());
        let setting = if label.eq_ignore_ascii_case("CapsLock") {
            Setting::CapsLock(
                capslock_action(value)
                    .ok_or_else(|| at("CapsLock takes easyshift, capslock, off or default"))?,
            )
        } else if label.eq_ignore_ascii_case("light") {
            Setting::Light(parse_light_value(value).ok_or_else(|| {
                at(
                    "light takes default, or brightness 0-5, dim 0-5 and timeout 1-255, \
                    separated by commas",
                )
            })?)
        } else {
            let (group, position) = find_key(label).ok_or_else(|| {
                at(&format!(
                    "{label} is not a key this tool can set, see the README"
                ))
            })?;
            let value = parse_key_value(value).ok_or_else(|| {
                at(&format!(
                    "{label} takes a shortcut such as Ctrl+F5, an action such as \
                     profile-cycle, default, or macro \"Ctrl+C, wait 50, Ctrl+V\""
                ))
            })?;
            if value == KeyValue::Default && group.defaults.is_none() {
                return Err(at(&format!(
                    "{label} has no default; default only works for zone, F- and media keys"
                )));
            }
            if matches!(value, KeyValue::Macro(..)) && macro_slot(group, position).is_none() {
                return Err(at(&format!("{label} cannot play a macro")));
            }
            Setting::Key {
                group,
                position,
                value,
            }
        };
        let repeated = settings.iter().any(|s| same_target(s, &setting));
        if repeated {
            return Err(at(&format!("{label} is already set earlier in the file")));
        }
        settings.push(setting);
    }
    Ok(settings)
}

/// True when two settings change the same key, Caps Lock or the lighting.
fn same_target(a: &Setting, b: &Setting) -> bool {
    match (a, b) {
        (
            Setting::Key {
                group: g1,
                position: p1,
                ..
            },
            Setting::Key {
                group: g2,
                position: p2,
                ..
            },
        ) => g1.report.id == g2.report.id && p1 == p2,
        (Setting::CapsLock(_), Setting::CapsLock(_)) | (Setting::Light(_), Setting::Light(_)) => {
            true
        }
        _ => false,
    }
}

/// True when a key already does what a value says, so `import` leaves its
/// bytes alone. A macro value only checks that the key plays its slot; the
/// macro in the slot is compared separately.
pub fn already_set(group: &KeyGroup, position: usize, binding: Binding, value: &KeyValue) -> bool {
    match value {
        KeyValue::Default => group
            .defaults
            .and_then(|d| d.get(position))
            .is_some_and(|d| *d == binding.action),
        KeyValue::Action(code) => same_action(binding.action, *code),
        KeyValue::Shortcut(modifier, key) => {
            binding.action == ACTION_SHORTCUT
                && binding.modifier == *modifier
                && binding.key == *key
        }
        KeyValue::Macro(..) => binding.action == ACTION_MACRO,
    }
}

/// Returns a copy of a key report with one key set to a value.
pub fn with_key_value(
    report: &[u8],
    group: &KeyGroup,
    position: usize,
    value: &KeyValue,
) -> Option<Vec<u8>> {
    match value {
        KeyValue::Default => with_default(report, group, position),
        KeyValue::Action(code) => with_action(report, group, position, *code),
        KeyValue::Shortcut(modifier, key) => {
            with_shortcut(report, group, position, *modifier, *key)
        }
        KeyValue::Macro(..) => with_macro(report, group, position),
    }
}

/// A key's value as a profile file writes it, or None when `import` could
/// not set it back exactly. Macros are written by `profile_text`.
fn binding_text(group: &KeyGroup, position: usize, binding: Binding) -> Option<String> {
    let value = if binding.action == ACTION_SHORTCUT {
        let text = shortcut_text(binding.modifier, binding.key);
        let reads_back = parse_shortcut(&text) == Some((binding.modifier, binding.key));
        return reads_back.then_some(text);
    } else if group.defaults.and_then(|d| d.get(position)) == Some(&binding.action) {
        "default"
    } else if is_easyshift(binding.action) {
        "easyshift"
    } else {
        ACTIONS
            .iter()
            .find(|(_, code)| *code == binding.action)
            .map(|(name, _)| *name)?
    };
    Some(value.to_string())
}

/// Caps Lock as a profile file writes it, or None for an action `import`
/// cannot set.
fn capslock_text(action: u8) -> Option<&'static str> {
    if is_easyshift(action) {
        return Some("easyshift");
    }
    CAPSLOCK_ACTIONS
        .iter()
        .find(|(name, code)| *code == action && *name != "default")
        .map(|(name, _)| *name)
}

/// How every line `export` writes for something `import` cannot set ends.
pub const LEFT_OUT_NOTE: &str = "import cannot set this, so it is left out.";

/// The groups a profile file lists, in order.
pub const PROFILE_TEXT_GROUPS: [&KeyGroup; 5] = [
    &THUMB_KEYS,
    &MACRO_KEYS,
    &EASYZONE_KEYS,
    &FUNCTION_KEYS,
    &MEDIA_KEYS,
];

/// Writes one profile as text. `reports` holds its key, Caps Lock and light
/// reports. `macros` holds the macro in each slot a key plays, or None for
/// a slot that holds none; a slot left out could not be read. What
/// `import` could not set back exactly is written as a `#` line ending in
/// LEFT_OUT_NOTE, so it is shown but skipped.
pub fn profile_text(
    profile: u8,
    reports: &[(Report, Vec<u8>)],
    macros: &[(u8, Option<Macro>)],
) -> Result<String, String> {
    let report = |wanted: &Report| {
        reports
            .iter()
            .find(|(r, _)| r.id == wanted.id)
            .map(|(_, bytes)| bytes.as_slice())
            .ok_or_else(|| format!("missing {} report", wanted.name))
    };
    let mut out = format!(
        "# Roccat Isku profile {}, saved by isku-tool export.\n\
         # Put it on a profile with: cargo run -- import <this file> <1-5>\n\
         # Each line takes the values set, macro and light take. import changes\n\
         # only what is listed, and skips lines that start with #.\n",
        profile + 1
    );
    for group in PROFILE_TEXT_GROUPS {
        let keys = bindings(report(&group.report)?, group.labels.len())
            .ok_or_else(|| format!("bad {} report", group.report.name))?;
        out.push_str(&format!("\n# {}\n", group.title));
        for (position, binding) in keys.into_iter().enumerate() {
            let label = key_label(group, position);
            let slot = macro_slot(group, position);
            let line = if binding.action == ACTION_MACRO && slot.is_some() {
                let found = macros.iter().find(|(s, _)| Some(*s) == slot);
                match found.map(|(_, m)| m.as_ref()) {
                    Some(Some(value)) => match macro_to_text(&value.keystrokes) {
                        Some(text) if plays_once(value) => Ok(format!("macro \"{text}\"")),
                        Some(_) => Err(format!(
                            "plays macro {}, set to repeat or with an unknown type",
                            describe_macro(value)
                        )),
                        None => Err(format!("plays macro {}", describe_macro(value))),
                    },
                    Some(None) => Err("plays a macro slot that holds none".to_string()),
                    None => Err("plays a macro this tool cannot read".to_string()),
                }
            } else {
                binding_text(group, position, binding).ok_or_else(|| describe(binding))
            };
            out.push_str(&match line {
                Ok(value) => format!("{label:<8} = {value}\n"),
                Err(what) => format!("# {label}: {what}. {LEFT_OUT_NOTE}\n"),
            });
        }
    }
    let capslock = capslock_of(report(&KEYS_CAPSLOCK)?).ok_or("bad Caps Lock report")?;
    out.push_str("\n# Caps Lock and lighting\n");
    out.push_str(&match capslock_text(capslock) {
        Some(value) => format!("{:<8} = {value}\n", "CapsLock"),
        None => format!(
            "# CapsLock: {}. {LEFT_OUT_NOTE}\n",
            describe_capslock(capslock)
        ),
    });
    let light = parse_light(report(&LIGHT)?).ok_or("bad light report")?;
    let light_text = format!(
        "brightness {}, dim {}, timeout {}",
        light.brightness, light.dimness, light.timeout
    );
    // Values out of range would make the file fail to import.
    out.push_str(&if light.is_valid() {
        format!("{:<8} = {light_text}\n", "light")
    } else {
        format!("# light: {light_text}, out of range. {LEFT_OUT_NOTE}\n")
    });
    Ok(out)
}

/// Applies the key, Caps Lock and lighting lines of a profile file to the
/// reports they change. Keys that already do what their line says keep
/// their bytes. Macros themselves are left to the caller, since they live
/// in their own slots.
pub fn apply_settings(
    reports: &mut [(Report, Vec<u8>)],
    settings: &[Setting],
) -> Result<(), String> {
    fn report_for<'a>(
        reports: &'a mut [(Report, Vec<u8>)],
        wanted: &Report,
    ) -> Result<&'a mut Vec<u8>, String> {
        reports
            .iter_mut()
            .find(|(r, _)| r.id == wanted.id)
            .map(|(_, bytes)| bytes)
            .ok_or_else(|| format!("missing {} report", wanted.name))
    }
    for setting in settings {
        match setting {
            Setting::Key {
                group,
                position,
                value,
            } => {
                let bytes = report_for(reports, &group.report)?;
                let binding = bindings(bytes, group.labels.len())
                    .and_then(|keys| keys.get(*position).copied())
                    .ok_or_else(|| format!("bad {} report", group.report.name))?;
                if !already_set(group, *position, binding, value) {
                    *bytes = with_key_value(bytes, group, *position, value)
                        .ok_or_else(|| format!("cannot set {}", key_label(group, *position)))?;
                }
            }
            Setting::CapsLock(action) => {
                let bytes = report_for(reports, &KEYS_CAPSLOCK)?;
                let current = capslock_of(bytes).ok_or("bad Caps Lock report")?;
                if !same_action(current, *action) {
                    *bytes = with_capslock(bytes, *action).ok_or("bad Caps Lock report")?;
                }
            }
            Setting::Light(change) => {
                let bytes = report_for(reports, &LIGHT)?;
                let light = parse_light(bytes).ok_or("bad light report")?;
                *bytes = with_light(bytes, change.apply(light)).ok_or(
                    "the profile's lighting is out of range now, so give the light line all \
                     three values, or default",
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::test_data::*;

    /// A report of the right size for profile 0, with every byte after the
    /// header zero and a correct checksum.
    fn blank(report: Report) -> Vec<u8> {
        let mut bytes = vec![0u8; report.len];
        bytes[0] = report.id;
        bytes[1] = report.len as u8;
        let body = bytes.len() - 2;
        let sum = checksum(&bytes[..body]);
        bytes[body..].copy_from_slice(&sum.to_le_bytes());
        bytes
    }

    /// Profile 1 as in the first probe: three thumb macros, three
    /// quicklaunch keys, the old Easy-Shift code on Caps Lock.
    fn profile() -> Vec<(Report, Vec<u8>)> {
        vec![
            (KEYS_THUMBSTER, THUMBSTER.to_vec()),
            (KEYS_MACRO, blank(KEYS_MACRO)),
            (KEYS_EASYZONE, easyzone()),
            (
                KEYS_FUNCTION,
                with_all_defaults(&blank(KEYS_FUNCTION), &FUNCTION_KEYS).unwrap(),
            ),
            (
                KEYS_MEDIA,
                with_all_defaults(&blank(KEYS_MEDIA), &MEDIA_KEYS).unwrap(),
            ),
            (KEYS_CAPSLOCK, CAPSLOCK.to_vec()),
            (LIGHT, LIGHT_REPORT.to_vec()),
        ]
    }

    /// A macro as the old software saved them: macro type, played once.
    fn sample(name: &str, strokes: &[(u8, u8)]) -> Option<Macro> {
        let mut value = parse_macro(&sample_macro(name, strokes))?;
        value.header = NEW_MACRO_HEADER;
        Some(value)
    }

    #[test]
    fn export_leaves_out_macros_that_repeat() {
        let mut macros = macros();
        let ctrl_q = macros[0].1.as_mut().unwrap();
        ctrl_q.header[1] = 3;
        let text = profile_text(0, &profile(), &macros).unwrap();
        assert!(text.contains("\n# T1: plays macro \"Ctrl q\""), "{text}");
        // A slot that could not be read is not mistaken for an empty one.
        macros.remove(0);
        let text = profile_text(0, &profile(), &macros).unwrap();
        assert!(text.contains("\n# T1: plays a macro this tool cannot read."));
    }

    fn macros() -> Vec<(u8, Option<Macro>)> {
        vec![
            (
                0x1e,
                sample("Ctrl q", &[(0xe0, 1), (0x14, 1), (0x14, 2), (0xe0, 2)]),
            ),
            // Shift held across two keys, which macro text cannot say.
            (
                0x1f,
                sample(
                    "AB",
                    &[
                        (0xe1, 1),
                        (0x04, 1),
                        (0x04, 2),
                        (0x05, 1),
                        (0x05, 2),
                        (0xe1, 2),
                    ],
                ),
            ),
            (0x20, None),
        ]
    }

    #[test]
    fn export_lists_every_key_and_comments_what_import_cannot_set() {
        let text = profile_text(0, &profile(), &macros()).unwrap();
        for line in [
            "T1       = macro \"Ctrl+Q\"",
            "# T2: plays macro \"AB\": press LeftShift, press A, release A, press B, \
             release B, release LeftShift. import cannot set this, so it is left out.",
            "# T3: plays a macro slot that holds none. import cannot set this, so it is left out.",
            "# Shift+T1: quicklaunch (0xb9). import cannot set this, so it is left out.",
            "M1       = off",
            "Shift+Q  = default",
            "F12      = default",
            "Computer = default",
            "CapsLock = easyshift",
            "light    = brightness 5, dim 1, timeout 15",
        ] {
            assert!(text.lines().any(|l| l == line), "{line}\n\n{text}");
        }
    }

    #[test]
    fn export_leaves_out_lighting_out_of_range() {
        let mut reports = profile();
        let light = &mut reports[6].1;
        light[7] = 0;
        let sum = checksum(&light[..8]);
        light[8..].copy_from_slice(&sum.to_le_bytes());
        let text = profile_text(0, &reports, &macros()).unwrap();
        assert!(
            text.contains("\n# light: brightness 5, dim 1, timeout 0, out of range."),
            "{text}"
        );
        let settings = parse_profile_text(&text).unwrap();
        let before = reports.clone();
        apply_settings(&mut reports, &settings).unwrap();
        assert_eq!(reports[6].1, before[6].1);
        // A line that sets only part of it cannot fix it.
        let partial = parse_profile_text("light = dim 2").unwrap();
        let error = apply_settings(&mut reports, &partial).unwrap_err();
        assert!(error.contains("out of range"), "{error}");
        let full = parse_profile_text("light = default").unwrap();
        apply_settings(&mut reports, &full).unwrap();
        assert_eq!(parse_light(&reports[6].1), Some(LIGHT_DEFAULT));
    }

    #[test]
    fn importing_an_export_changes_nothing() {
        let before = profile();
        let text = profile_text(0, &before, &macros()).unwrap();
        let settings = parse_profile_text(&text).unwrap();
        // 6 thumb keys minus 5 commented, 10 M, 20 zone, 12 F, 8 media,
        // Caps Lock and light.
        assert_eq!(settings.len(), 1 + 10 + 20 + 12 + 8 + 2);
        let mut after = before.clone();
        apply_settings(&mut after, &settings).unwrap();
        for ((report, a), (_, b)) in after.iter().zip(&before) {
            assert_eq!(a, b, "{}", report.name);
        }
    }

    #[test]
    fn import_changes_what_a_file_says() {
        let text = "\u{feff}# a comment\n\n\
                    t2 = Ctrl+F5\n\
                    Shift+T1 = macro Ctrl+C, wait 50, Ctrl+V\n\
                    shift+q = profile-cycle\n\
                    F1=off\n\
                    capslock = off\n\
                    light = dim 0, brightness 2\n";
        let settings = parse_profile_text(text).unwrap();
        assert_eq!(settings.len(), 6);
        let mut reports = profile();
        apply_settings(&mut reports, &settings).unwrap();
        let keys = |group: &KeyGroup| {
            let (_, bytes) = reports
                .iter()
                .find(|(r, _)| r.id == group.report.id)
                .unwrap();
            assert!(checksum_ok(bytes));
            bindings(bytes, group.labels.len()).unwrap()
        };
        let thumb = keys(&THUMB_KEYS);
        assert_eq!(
            thumb[1],
            Binding {
                action: ACTION_SHORTCUT,
                modifier: 1 << 2,
                key: 0x3e
            }
        );
        assert_eq!(thumb[3].action, ACTION_MACRO);
        assert_eq!(thumb[0].action, ACTION_MACRO, "T1 is not in the file");
        assert_eq!(keys(&EASYZONE_KEYS)[5].action, 0xb2);
        assert_eq!(keys(&FUNCTION_KEYS)[0].action, ACTION_OFF);
        let find = |id: u8| &reports.iter().find(|(r, _)| r.id == id).unwrap().1;
        assert_eq!(capslock_of(find(KEYS_CAPSLOCK.id)), Some(ACTION_OFF));
        assert_eq!(
            parse_light(find(LIGHT.id)),
            Some(Light {
                brightness: 2,
                dimness: 0,
                timeout: 15
            })
        );
        let Setting::Key { value, .. } = &settings[1] else {
            panic!("{:?}", settings[1]);
        };
        assert_eq!(
            *value,
            KeyValue::Macro(
                "Ctrl+C, wait 50, Ctrl+V".to_string(),
                parse_macro_text("Ctrl+C, wait 50, Ctrl+V").unwrap()
            )
        );
    }

    #[test]
    fn bad_lines_are_refused_with_their_number() {
        for (text, error) in [
            ("T1 Ctrl+F5", "line 1: write it as key = value"),
            ("\nT9 = 7", "line 2: T9 is not a key"),
            ("T1 = Ctrl+", "line 1: T1 takes a shortcut"),
            ("T1 = macro", "line 1: T1 takes a shortcut"),
            ("T1 = macro \"\"", "line 1: T1 takes a shortcut"),
            ("T1 = default", "line 1: T1 has no default"),
            ("F1 = macro 7", "line 1: F1 cannot play a macro"),
            ("T1 = 7\nt1 = 8", "line 2: t1 is already set"),
            ("light = 3\n", "line 1: light takes"),
            ("light = brightness 6", "line 1: light takes"),
            ("light = timeout 0", "line 1: light takes"),
            ("light = dim 1, dim 2", "line 1: light takes"),
            ("light = brightness -1", "line 1: light takes"),
            ("CapsLock = Ctrl+A", "line 1: CapsLock takes"),
            (
                "CapsLock = off\nCAPSLOCK = off",
                "line 2: CAPSLOCK is already set",
            ),
        ] {
            let got = parse_profile_text(text).unwrap_err();
            assert!(got.starts_with(error), "{text:?}: {got}");
        }
    }

    #[test]
    fn easyshift_keeps_the_old_code() {
        let mut reports = profile();
        let settings = parse_profile_text("CapsLock = easyshift\nT1 = easyshift").unwrap();
        apply_settings(&mut reports, &settings).unwrap();
        let (_, caps) = &reports[5];
        assert_eq!(caps.as_slice(), CAPSLOCK.as_slice());
        let (_, thumb) = &reports[0];
        assert_eq!(bindings(thumb, 6).unwrap()[0].action, ACTION_EASYSHIFT);
    }
}

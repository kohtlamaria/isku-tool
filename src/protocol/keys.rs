//! Key groups, key names, shortcuts and editing key reports.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub action: u8,
    pub modifier: u8,
    pub key: u8,
}

/// Splits a key report into per-key bindings. The report stores three arrays
/// after the header: all actions, then all modifiers, then all shortcut keys.
pub fn bindings(report: &[u8], keys: usize) -> Option<Vec<Binding>> {
    let actions = report.get(3..3 + keys)?;
    let modifiers = report.get(3 + keys..3 + 2 * keys)?;
    let shortcut_keys = report.get(3 + 2 * keys..3 + 3 * keys)?;
    Some(
        (0..keys)
            .map(|i| Binding {
                action: actions[i],
                modifier: modifiers[i],
                key: shortcut_keys[i],
            })
            .collect(),
    )
}

#[derive(Clone, Copy, Debug)]
pub struct KeyGroup {
    pub title: &'static str,
    pub report: Report,
    pub labels: &'static [&'static str],
    /// The first key's index. Keys below 0x24 can hold macros.
    pub first_macro_index: u8,
    /// What each key holds when it has not been changed, for groups that have one.
    pub defaults: Option<&'static [u8]>,
}

pub const THUMB_KEYS: KeyGroup = KeyGroup {
    title: "Thumb keys",
    report: KEYS_THUMBSTER,
    labels: &["T1", "T2", "T3", "Shift+T1", "Shift+T2", "Shift+T3"],
    first_macro_index: 0x1e,
    defaults: None,
};

pub const MACRO_KEYS: KeyGroup = KeyGroup {
    title: "Macro keys",
    report: KEYS_MACRO,
    labels: &[
        "M1", "M2", "M3", "M4", "M5", "Shift+M1", "Shift+M2", "Shift+M3", "Shift+M4", "Shift+M5",
    ],
    first_macro_index: 0x00,
    defaults: None,
};

pub const EASYZONE_KEYS: KeyGroup = KeyGroup {
    title: "Easy-Shift zone",
    report: KEYS_EASYZONE,
    labels: &[
        "1", "2", "3", "4", "5", "Q", "W", "E", "R", "T", "A", "S", "D", "F", "G", "Z", "X", "C",
        "V", "B",
    ],
    first_macro_index: 0x0a,
    defaults: Some(&EASYZONE_DEFAULTS),
};

/// What the Easy-Shift zone holds when a key has not been changed.
pub const EASYZONE_DEFAULTS: [u8; 20] = [
    0x1e, 0x1f, 0x20, 0x21, 0x22, 0x14, 0x1a, 0x08, 0x15, 0x17, 0x04, 0x16, 0x07, 0x09, 0x0a, 0x1d,
    0x1b, 0x06, 0x19, 0x05,
];

pub const FUNCTION_KEYS: KeyGroup = KeyGroup {
    title: "F-keys",
    report: KEYS_FUNCTION,
    labels: &[
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
    ],
    first_macro_index: 0x24,
    defaults: Some(&[
        0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45,
    ]),
};

/// Names and defaults from roccat-tools, matching profiles 3-5 in our backup.
pub const MEDIA_KEYS: KeyGroup = KeyGroup {
    title: "Media keys",
    report: KEYS_MEDIA,
    labels: &[
        "Mute", "VolDown", "VolUp", "Play", "Prev", "Next", "WWW", "Computer",
    ],
    first_macro_index: 0x30,
    defaults: Some(&[0xa0, 0xa2, 0xa1, 0xa4, 0xa5, 0xa6, 0xa8, 0xa9]),
};

/// Named keys that are not letters, numbers or F-keys. Codes are USB HID usage
/// IDs, which name a key's position on a US layout. On an Estonian layout the
/// same positions print different symbols, for example Minus is the + key.
const NAMED_KEYS: &[(u8, &str)] = &[
    (0x28, "Enter"),
    (0x29, "Esc"),
    (0x2a, "Backspace"),
    (0x2b, "Tab"),
    (0x2c, "Space"),
    (0x2d, "Minus"),
    (0x2e, "Equal"),
    (0x2f, "LeftBracket"),
    (0x30, "RightBracket"),
    (0x31, "Backslash"),
    (0x33, "Semicolon"),
    (0x34, "Quote"),
    (0x35, "Grave"),
    (0x36, "Comma"),
    (0x37, "Period"),
    (0x38, "Slash"),
    (0x39, "CapsLock"),
    (0x46, "PrintScreen"),
    (0x47, "ScrollLock"),
    (0x48, "Pause"),
    (0x49, "Insert"),
    (0x4a, "Home"),
    (0x4b, "PageUp"),
    (0x4c, "Delete"),
    (0x4d, "End"),
    (0x4e, "PageDown"),
    (0x4f, "Right"),
    (0x50, "Left"),
    (0x51, "Down"),
    (0x52, "Up"),
    (0x53, "NumLock"),
    (0x54, "NumSlash"),
    (0x55, "NumStar"),
    (0x56, "NumMinus"),
    (0x57, "NumPlus"),
    (0x58, "NumEnter"),
    (0x63, "NumPeriod"),
    (0xe0, "LeftCtrl"),
    (0xe1, "LeftShift"),
    (0xe2, "LeftAlt"),
    (0xe3, "LeftWin"),
    (0xe4, "RightCtrl"),
    (0xe5, "RightShift"),
    (0xe6, "RightAlt"),
    (0xe7, "RightWin"),
    (0x64, "IsoBackslash"),
    (0x65, "Menu"),
];

/// Names for HID keyboard usage codes.
pub fn key_name(code: u8) -> String {
    match code {
        0x04..=0x1d => char::from(b'A' + (code - 0x04)).to_string(),
        0x1e..=0x26 => (code - 0x1d).to_string(),
        0x27 => "0".to_string(),
        0x3a..=0x45 => format!("F{}", code - 0x39),
        0x59..=0x61 => format!("Num{}", code - 0x58),
        0x62 => "Num0".to_string(),
        0x68..=0x73 => format!("F{}", code - 0x68 + 13),
        _ => NAMED_KEYS.iter().find(|(c, _)| *c == code).map_or_else(
            || format!("key 0x{code:02x}"),
            |(_, name)| (*name).to_string(),
        ),
    }
}

/// The HID code for a key name, ignoring case: `A`, `7`, `F5`, `Space`, `Num1`.
pub fn key_code(name: &str) -> Option<u8> {
    let upper = name.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    match bytes {
        [c @ b'A'..=b'Z'] => return Some(0x04 + (c - b'A')),
        [c @ b'1'..=b'9'] => return Some(0x1e + (c - b'1')),
        [b'0'] => return Some(0x27),
        _ => {}
    }
    if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
        return match n {
            1..=12 => Some(0x39 + n),
            13..=24 => Some(0x68 + n - 13),
            _ => None,
        };
    }
    if let Some(n) = upper.strip_prefix("NUM").and_then(|n| n.parse::<u8>().ok()) {
        return match n {
            0 => Some(0x62),
            1..=9 => Some(0x58 + n),
            _ => None,
        };
    }
    NAMED_KEYS
        .iter()
        .find(|(_, n)| n.eq_ignore_ascii_case(name))
        .map(|(code, _)| *code)
}

const MODIFIERS: [(u8, &str); 4] = [(2, "Ctrl"), (1, "Shift"), (3, "Alt"), (4, "Win")];

/// Reads a shortcut such as `7`, `F5` or `Ctrl+Alt+Z` into a modifier mask and
/// a key code. Each modifier may appear once, and the key comes last.
pub fn parse_shortcut(text: &str) -> Option<(u8, u8)> {
    let mut parts: Vec<&str> = text.split('+').collect();
    let key = key_code(parts.pop()?)?;
    let mut modifier = 0u8;
    for part in parts {
        let (bit, _) = MODIFIERS
            .iter()
            .find(|(_, name)| name.eq_ignore_ascii_case(part))?;
        if modifier & (1 << bit) != 0 {
            return None;
        }
        modifier |= 1 << bit;
    }
    Some((modifier, key))
}

/// Writes a shortcut the way `parse_shortcut` reads it.
pub fn shortcut_text(modifier: u8, key: u8) -> String {
    let mut text = String::new();
    for (bit, name) in MODIFIERS {
        if modifier & (1 << bit) != 0 {
            text.push_str(name);
            text.push('+');
        }
    }
    text + &key_name(key)
}

fn action_name(action: u8) -> Option<&'static str> {
    Some(match action {
        0x00 => "disabled",
        0x81 => "system shut down",
        0x82 => "system sleep",
        0x83 => "system wake",
        0x8c => "Easy-Shift",
        0xa0 => "mute",
        0xa1 => "volume up",
        0xa2 => "volume down",
        0xa4 => "play/pause",
        0xa5 => "previous track",
        0xa6 => "next track",
        0xa8 => "open browser",
        0xa9 => "open computer",
        0xaa..=0xae => "M key",
        0xb0 => "profile up",
        0xb1 => "profile down",
        0xb2 => "profile cycle",
        0xb3..=0xb7 => "switch to profile",
        0xb9 => "quicklaunch",
        0xba => "macro play/pause",
        0xbc => "timer",
        0xbd => "timer stop",
        0xbe => "macro",
        0xd3..=0xd7 => "standard Shift+number",
        0xd8..=0xdc => "standard Ctrl+number",
        0xdd => "open driver",
        0xc0 => "open email",
        0xc1 => "open calculator",
        0xc2 => "open computer",
        0xc3 => "open browser",
        0xc4 => "web search",
        0xc5 => "web home",
        0xc6 => "web back",
        0xc7 => "web forward",
        0xc8 => "web stop",
        0xc9 => "web refresh",
        0xca => "web favorites",
        0xcb => "open media player",
        0xcc => "play/pause",
        0xcd => "next track",
        0xce => "previous track",
        0xcf => "stop",
        0xd0 => "volume up",
        0xd1 => "volume down",
        0xd2 => "mute",
        0xdf => "unused",
        0xef => "standard",
        0xfc..=0xfe => "T key",
        0xff => "Easy-Shift",
        _ => return None,
    })
}

pub fn describe(binding: Binding) -> String {
    if binding.action == ACTION_SHORTCUT {
        return format!("shortcut {}", shortcut_text(binding.modifier, binding.key));
    }
    match action_name(binding.action) {
        Some(name) => format!("{name} (0x{:02x})", binding.action),
        None if binding.action < 0x80 => format!("key {}", key_name(binding.action)),
        None => format!("unknown action (0x{:02x})", binding.action),
    }
}

/// Finds a settable key by name: `T1`, `Shift+T1`, `M1`, `Shift+M1`, `F1`,
/// `Mute`, or an Easy-Shift zone key such as `Shift+Q`.
pub fn find_key(label: &str) -> Option<(&'static KeyGroup, usize)> {
    for group in [&THUMB_KEYS, &MACRO_KEYS, &FUNCTION_KEYS, &MEDIA_KEYS] {
        if let Some(i) = group
            .labels
            .iter()
            .position(|l| l.eq_ignore_ascii_case(label))
        {
            return Some((group, i));
        }
    }
    let (prefix, key) = label.split_at_checked(6)?;
    if !prefix.eq_ignore_ascii_case("Shift+") {
        return None;
    }
    let i = EASYZONE_KEYS
        .labels
        .iter()
        .position(|l| l.eq_ignore_ascii_case(key))?;
    Some((&EASYZONE_KEYS, i))
}

/// Returns a copy of a key report with one key set to a shortcut, and the
/// checksum updated.
pub fn with_shortcut(
    report: &[u8],
    group: &KeyGroup,
    position: usize,
    modifier: u8,
    key: u8,
) -> Option<Vec<u8>> {
    with_binding(
        report,
        group,
        position,
        Binding {
            action: ACTION_SHORTCUT,
            modifier,
            key,
        },
    )
}

/// Returns a copy of a key report with one key put back to what it does when
/// unchanged, for groups that have defaults.
pub fn with_default(report: &[u8], group: &KeyGroup, position: usize) -> Option<Vec<u8>> {
    let action = *group.defaults?.get(position)?;
    with_binding(
        report,
        group,
        position,
        Binding {
            action,
            modifier: 0,
            key: 0,
        },
    )
}

/// Returns a copy of a key report with every key put back to normal, for
/// groups that have defaults.
pub fn with_all_defaults(report: &[u8], group: &KeyGroup) -> Option<Vec<u8>> {
    let mut updated = report.to_vec();
    for position in 0..group.labels.len() {
        updated = with_default(&updated, group, position)?;
    }
    Some(updated)
}

/// Caps Lock as Easy-Shift, which is the factory setting in roccat-tools.
pub const CAPSLOCK_DEFAULT: u8 = 0xff;

/// What `set <profile> CapsLock ...` accepts. The Caps Lock report has one
/// action byte and no room for a shortcut.
pub const CAPSLOCK_ACTIONS: &[(&str, u8)] = &[
    ("easyshift", 0xff),
    ("capslock", 0x39),
    ("off", 0x00),
    ("default", CAPSLOCK_DEFAULT),
];

pub fn capslock_action(name: &str) -> Option<u8> {
    CAPSLOCK_ACTIONS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, code)| *code)
}

pub fn describe_capslock(action: u8) -> String {
    match action {
        0x39 => "normal Caps Lock (0x39)".to_string(),
        _ => describe(Binding {
            action,
            modifier: 0,
            key: 0,
        }),
    }
}

/// The Caps Lock action byte of a valid Caps Lock report.
pub fn capslock_of(report: &[u8]) -> Option<u8> {
    (report.len() == KEYS_CAPSLOCK.len && report[0] == KEYS_CAPSLOCK.id).then(|| report[3])
}

/// Returns a copy of a Caps Lock report with a new action and checksum.
pub fn with_capslock(report: &[u8], action: u8) -> Option<Vec<u8>> {
    if report.len() != KEYS_CAPSLOCK.len || report[0] != KEYS_CAPSLOCK.id {
        return None;
    }
    let mut updated = report.to_vec();
    updated[3] = action;
    let sum = checksum(&updated[..4]);
    updated[4..].copy_from_slice(&sum.to_le_bytes());
    Some(updated)
}

pub(crate) fn with_binding(
    report: &[u8],
    group: &KeyGroup,
    position: usize,
    binding: Binding,
) -> Option<Vec<u8>> {
    let keys = group.labels.len();
    if position >= keys || report.len() != group.report.len {
        return None;
    }
    let mut updated = report.to_vec();
    updated[3 + position] = binding.action;
    updated[3 + keys + position] = binding.modifier;
    updated[3 + 2 * keys + position] = binding.key;
    let body = updated.len() - 2;
    let sum = checksum(&updated[..body]);
    updated[body..].copy_from_slice(&sum.to_le_bytes());
    Some(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::test_data::*;

    #[test]
    fn key_report_sizes_fit_their_key_counts() {
        for group in [
            THUMB_KEYS,
            MACRO_KEYS,
            EASYZONE_KEYS,
            FUNCTION_KEYS,
            MEDIA_KEYS,
        ] {
            assert_eq!(group.report.len, 5 + 3 * group.labels.len());
            if let Some(defaults) = group.defaults {
                assert_eq!(defaults.len(), group.labels.len());
            }
        }
    }

    #[test]
    fn bindings_split_the_three_arrays() {
        let mut report = THUMBSTER;
        report[3] = ACTION_SHORTCUT;
        report[9] = 0x02;
        report[15] = 0x1e;
        let keys = bindings(&report, 6).unwrap();
        assert_eq!(
            keys[0],
            Binding {
                action: 0xb8,
                modifier: 0x02,
                key: 0x1e
            }
        );
        assert_eq!(
            keys[5],
            Binding {
                action: 0xb9,
                modifier: 0,
                key: 0
            }
        );
        assert_eq!(bindings(&CAPSLOCK, 6), None);
    }

    #[test]
    fn describe_names_actions() {
        let bind = |action, modifier, key| Binding {
            action,
            modifier,
            key,
        };
        assert_eq!(describe(bind(0xbe, 0, 0)), "macro (0xbe)");
        assert_eq!(describe(bind(0x1e, 0, 0)), "key 1");
        assert_eq!(describe(bind(0x27, 0, 0)), "key 0");
        assert_eq!(describe(bind(0xb8, 0b0110, 0x1e)), "shortcut Ctrl+Shift+1");
    }

    #[test]
    fn key_names_cover_letters_numbers_and_function_keys() {
        assert_eq!(key_name(0x04), "A");
        assert_eq!(key_name(0x1d), "Z");
        assert_eq!(key_name(0x26), "9");
        assert_eq!(key_name(0x45), "F12");
    }

    #[test]
    fn with_shortcut_changes_one_key_and_keeps_the_report_valid() {
        let updated = with_shortcut(&THUMBSTER, &THUMB_KEYS, 0, 0, 0x24).unwrap();
        assert!(checksum_ok(&updated));
        let keys = bindings(&updated, 6).unwrap();
        assert_eq!(
            keys[0],
            Binding {
                action: ACTION_SHORTCUT,
                modifier: 0,
                key: 0x24
            }
        );
        assert_eq!(keys[1], bindings(&THUMBSTER, 6).unwrap()[1]);
        assert_eq!(describe(keys[0]), "shortcut 7");
    }

    #[test]
    fn with_shortcut_refuses_a_key_outside_the_group() {
        assert_eq!(with_shortcut(&THUMBSTER, &THUMB_KEYS, 6, 0, 0x24), None);
    }

    #[test]
    fn parse_shortcut_reads_keys_and_modifiers() {
        assert_eq!(parse_shortcut("7"), Some((0, 0x24)));
        assert_eq!(parse_shortcut("f5"), Some((0, 0x3e)));
        assert_eq!(parse_shortcut("Space"), Some((0, 0x2c)));
        assert_eq!(parse_shortcut("Ctrl+1"), Some((0b0100, 0x1e)));
        assert_eq!(parse_shortcut("ctrl+alt+z"), Some((0b1100, 0x1d)));
        assert_eq!(parse_shortcut("Win+Shift+F24"), Some((0b10010, 0x73)));
        assert_eq!(parse_shortcut("Num0"), Some((0, 0x62)));
    }

    #[test]
    fn parse_shortcut_refuses_anything_unclear() {
        for text in [
            "",
            "Ctrl+",
            "Ctrl+Ctrl+1",
            "Ctrl",
            "Foo",
            "F25",
            "F0",
            "Hyper+1",
            "1+Ctrl",
            "Num10",
        ] {
            assert_eq!(parse_shortcut(text), None, "{text}");
        }
    }

    #[test]
    fn every_named_key_reads_back_as_itself() {
        for code in (0x04..=0x65).chain(0x68..=0x73).chain(0xe0..=0xe7) {
            let name = key_name(code);
            if !name.starts_with("key ") {
                assert_eq!(key_code(&name), Some(code), "{name}");
            }
        }
        assert_eq!(shortcut_text(0b1100, 0x1d), "Ctrl+Alt+Z");
        assert_eq!(
            parse_shortcut(&shortcut_text(0b11110, 0x3e)),
            Some((0b11110, 0x3e))
        );
    }

    #[test]
    fn easyzone_sample_matches_the_dump() {
        assert!(valid_profile_report(&KEYS_EASYZONE, 0, &easyzone()));
    }

    #[test]
    fn easyzone_default_undoes_a_shortcut() {
        let changed = with_shortcut(&easyzone(), &EASYZONE_KEYS, 5, 0b0100, 0x1e).unwrap();
        assert!(checksum_ok(&changed));
        assert_eq!(
            describe(bindings(&changed, 20).unwrap()[5]),
            "shortcut Ctrl+1"
        );
        assert_eq!(
            with_default(&changed, &EASYZONE_KEYS, 5).unwrap(),
            easyzone()
        );
        assert_eq!(with_default(&changed, &EASYZONE_KEYS, 20), None);
    }

    #[test]
    fn find_key_names_each_group() {
        let found = |label| find_key(label).map(|(g, i)| (g.report.id, i));
        assert_eq!(found("T1"), Some((0x0b, 0)));
        assert_eq!(found("shift+t3"), Some((0x0b, 5)));
        assert_eq!(found("M5"), Some((0x0d, 4)));
        assert_eq!(found("Shift+M1"), Some((0x0d, 5)));
        assert_eq!(found("Shift+Q"), Some((0x09, 5)));
        assert_eq!(found("Shift+1"), Some((0x09, 0)));
        assert_eq!(found("shift+b"), Some((0x09, 19)));
        assert_eq!(found("Q"), None);
        assert_eq!(found("Shift+P"), None);
        assert_eq!(found("Shift+"), None);
        assert_eq!(found("Shift+ä"), None);
    }

    #[test]
    fn find_key_names_f_keys_and_media_keys() {
        let found = |label| find_key(label).map(|(g, i)| (g.report.id, i));
        assert_eq!(found("F1"), Some((0x08, 0)));
        assert_eq!(found("f12"), Some((0x08, 11)));
        assert_eq!(found("Mute"), Some((0x0a, 0)));
        assert_eq!(found("volup"), Some((0x0a, 2)));
        assert_eq!(found("Computer"), Some((0x0a, 7)));
        assert_eq!(found("F13"), None);
    }

    #[test]
    fn media_default_restores_profile_3_bytes() {
        // Real profile 3 media report from the backup.
        let original = [
            0x0a, 0x1d, 0x02, 0xa0, 0xa2, 0xa1, 0xa4, 0xa5, 0xa6, 0xa8, 0xa9, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0x4c, 0x05,
        ];
        assert!(valid_profile_report(&KEYS_MEDIA, 2, &original));
        let changed = with_shortcut(&original, &MEDIA_KEYS, 0, 0, 0x3e).unwrap();
        assert_eq!(describe(bindings(&changed, 8).unwrap()[0]), "shortcut F5");
        assert_eq!(with_default(&changed, &MEDIA_KEYS, 0).unwrap(), original);
        assert_eq!(with_default(&changed, &THUMB_KEYS, 0), None);
    }

    #[test]
    fn all_defaults_resets_every_key_in_a_group() {
        let mut changed = with_shortcut(&easyzone(), &EASYZONE_KEYS, 0, 0b0100, 0x1e).unwrap();
        changed = with_shortcut(&changed, &EASYZONE_KEYS, 19, 0, 0x3e).unwrap();
        assert_eq!(
            with_all_defaults(&changed, &EASYZONE_KEYS).unwrap(),
            easyzone()
        );
        assert_eq!(with_all_defaults(&THUMBSTER, &THUMB_KEYS), None);
    }

    #[test]
    fn capslock_matches_real_reports() {
        // Profile 1 had 8c and profile 5 had ff in the backup.
        let profile_5 = [0x13, 0x06, 0x04, 0xff, 0x1c, 0x01];
        let mut from_8c = CAPSLOCK;
        from_8c[2] = 0x04;
        let sum = checksum(&from_8c[..4]);
        from_8c[4..].copy_from_slice(&sum.to_le_bytes());
        assert_eq!(
            with_capslock(&from_8c, CAPSLOCK_DEFAULT).unwrap(),
            profile_5
        );
        assert!(valid_profile_report(&KEYS_CAPSLOCK, 4, &profile_5));
        assert_eq!(with_capslock(&THUMBSTER, CAPSLOCK_DEFAULT), None);
    }

    #[test]
    fn capslock_actions_by_name() {
        assert_eq!(capslock_action("EasyShift"), Some(0xff));
        assert_eq!(capslock_action("capslock"), Some(0x39));
        assert_eq!(capslock_action("off"), Some(0x00));
        assert_eq!(capslock_action("default"), Some(CAPSLOCK_DEFAULT));
        assert_eq!(capslock_action("Ctrl+F5"), None);
        assert_eq!(capslock_of(&CAPSLOCK), Some(0x8c));
        assert_eq!(capslock_of(&THUMBSTER), None);
        let off = with_capslock(&CAPSLOCK, 0x00).unwrap();
        assert_eq!(capslock_of(&off), Some(0x00));
        assert!(valid_profile_report(&KEYS_CAPSLOCK, 0, &off));
    }
}

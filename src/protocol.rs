//! The Isku report format. No USB code here, so all of this can be unit tested.
//! Constants come from the Linux driver and roccat-tools. See docs/protocol.md.

pub const VENDOR_ID: u16 = 0x1E7D;
pub const PRODUCT_ID: u16 = 0x319C;
/// The settings live on this interface.
pub const SETTINGS_USAGE_PAGE: u16 = 0x000B;
/// Windows reports this as the feature report length for the settings interface.
pub const FEATURE_BUFFER_LEN: usize = 2083;

pub const PROFILE_COUNT: u8 = 5;
pub const CONTROL_REPORT_ID: u8 = 0x04;
pub const MACRO_REPORT_ID: u8 = 0x0e;
pub const STATUS_OK: u8 = 1;
pub const STATUS_BUSY: u8 = 3;
/// Keys 0x00 to 0x23 can hold macros, and selecting one uses its index as the request.
pub const LAST_MACRO_KEY_INDEX: u8 = 0x23;

pub const ACTION_SHORTCUT: u8 = 0xb8;
pub const ACTION_MACRO: u8 = 0xbe;

#[derive(Clone, Copy, Debug)]
pub struct Report {
    pub id: u8,
    pub len: usize,
    pub name: &'static str,
    /// The control request that selects this report for a profile.
    pub request: Option<u8>,
    pub has_checksum: bool,
}

const fn report(id: u8, len: usize, name: &'static str, request: Option<u8>, has_checksum: bool) -> Report {
    Report { id, len, name, request, has_checksum }
}

pub const ACTUAL_PROFILE: Report = report(0x05, 3, "actual_profile", None, false);
pub const INFO: Report = report(0x0f, 6, "info", None, false);
pub const LAST_SET: Report = report(0x14, 20, "last_set", None, true);

pub const KEY_MASK: Report = report(0x07, 6, "key_mask", Some(0x50), true);
pub const KEYS_FUNCTION: Report = report(0x08, 41, "keys_function", Some(0x60), true);
pub const LIGHT: Report = report(0x10, 10, "light", Some(0x70), true);
pub const KEYS_CAPSLOCK: Report = report(0x13, 6, "keys_capslock", Some(0x90), true);
pub const KEYS_MACRO: Report = report(0x0d, 35, "keys_macro", Some(0xa0), true);
pub const KEYS_EASYZONE: Report = report(0x09, 65, "keys_easyzone", Some(0xb0), true);
pub const KEYS_MEDIA: Report = report(0x0a, 29, "keys_media", Some(0xc0), true);
pub const KEYS_THUMBSTER: Report = report(0x0b, 23, "keys_thumbster", Some(0xd0), true);

/// Every report that exists once per profile.
pub const PROFILE_REPORTS: [Report; 8] = [
    KEY_MASK, KEYS_FUNCTION, LIGHT, KEYS_CAPSLOCK, KEYS_MACRO, KEYS_EASYZONE, KEYS_MEDIA,
    KEYS_THUMBSTER,
];

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
    labels: &["M1", "M2", "M3", "M4", "M5", "Shift+M1", "Shift+M2", "Shift+M3", "Shift+M4", "Shift+M5"],
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
    0x1e, 0x1f, 0x20, 0x21, 0x22, 0x14, 0x1a, 0x08, 0x15, 0x17, 0x04, 0x16, 0x07, 0x09, 0x0a,
    0x1d, 0x1b, 0x06, 0x19, 0x05,
];

pub const FUNCTION_KEYS: KeyGroup = KeyGroup {
    title: "F-keys",
    report: KEYS_FUNCTION,
    labels: &["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"],
    first_macro_index: 0x24,
    defaults: Some(&[0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45]),
};

/// Names and defaults from roccat-tools, matching profiles 3-5 in our backup.
pub const MEDIA_KEYS: KeyGroup = KeyGroup {
    title: "Media keys",
    report: KEYS_MEDIA,
    labels: &["Mute", "VolDown", "VolUp", "Play", "Prev", "Next", "WWW", "Computer"],
    first_macro_index: 0x30,
    defaults: Some(&[0xa0, 0xa2, 0xa1, 0xa4, 0xa5, 0xa6, 0xa8, 0xa9]),
};

/// The 16-bit sum of every byte, which the keyboard stores little-endian.
pub fn checksum(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0u16, |sum, b| sum.wrapping_add(u16::from(*b)))
}

/// True when the last two bytes are the checksum of everything before them.
pub fn checksum_ok(report: &[u8]) -> bool {
    let Some((body, stored)) = report.split_last_chunk::<2>() else {
        return false;
    };
    checksum(body) == u16::from_le_bytes(*stored)
}

/// Builds the "select this" command. Anything else returns None, so this is the
/// only shape of data the tool can send to the keyboard.
pub fn control_report(profile: u8, request: u8) -> Option<[u8; 3]> {
    let known_request = request <= LAST_MACRO_KEY_INDEX
        || PROFILE_REPORTS.iter().any(|r| r.request == Some(request));
    (profile < PROFILE_COUNT && known_request).then_some([CONTROL_REPORT_ID, profile, request])
}

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
            .map(|i| Binding { action: actions[i], modifier: modifiers[i], key: shortcut_keys[i] })
            .collect(),
    )
}

/// Names for HID keyboard usage codes.
/// Named keys that are not letters, numbers or F-keys. Codes are USB HID usage
/// IDs, which name a key's position on a US layout. On an Estonian layout the
/// same positions print different symbols, for example Minus is the + key.
const NAMED_KEYS: &[(u8, &str)] = &[
    (0x28, "Enter"), (0x29, "Esc"), (0x2a, "Backspace"), (0x2b, "Tab"), (0x2c, "Space"),
    (0x2d, "Minus"), (0x2e, "Equal"), (0x2f, "LeftBracket"), (0x30, "RightBracket"),
    (0x31, "Backslash"), (0x33, "Semicolon"), (0x34, "Quote"), (0x35, "Grave"),
    (0x36, "Comma"), (0x37, "Period"), (0x38, "Slash"), (0x39, "CapsLock"),
    (0x46, "PrintScreen"), (0x47, "ScrollLock"), (0x48, "Pause"), (0x49, "Insert"),
    (0x4a, "Home"), (0x4b, "PageUp"), (0x4c, "Delete"), (0x4d, "End"), (0x4e, "PageDown"),
    (0x4f, "Right"), (0x50, "Left"), (0x51, "Down"), (0x52, "Up"), (0x53, "NumLock"),
    (0x54, "NumSlash"), (0x55, "NumStar"), (0x56, "NumMinus"), (0x57, "NumPlus"),
    (0x58, "NumEnter"), (0x63, "NumPeriod"), (0x64, "IsoBackslash"), (0x65, "Menu"),
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
        _ => NAMED_KEYS
            .iter()
            .find(|(c, _)| *c == code)
            .map_or_else(|| format!("key 0x{code:02x}"), |(_, name)| (*name).to_string()),
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
        let (bit, _) = MODIFIERS.iter().find(|(_, name)| name.eq_ignore_ascii_case(part))?;
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
        None => format!("key {}", key_name(binding.action)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keystroke {
    pub key: u8,
    pub action: u8,
    pub period_ms: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Macro {
    pub macroset: String,
    pub name: String,
    pub keystrokes: Vec<Keystroke>,
}

fn text_at(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Reads a macro report, or returns None if it is not a valid one.
pub fn parse_macro(report: &[u8]) -> Option<Macro> {
    const NAMES_AT: usize = 31;
    const NAME_LEN: usize = 24;
    const COUNT_AT: usize = NAMES_AT + 2 * NAME_LEN;
    const KEYSTROKES_AT: usize = COUNT_AT + 2;

    if report.len() != FEATURE_BUFFER_LEN || report[0] != MACRO_REPORT_ID || !checksum_ok(report) {
        return None;
    }
    let count = usize::from(u16::from_le_bytes([report[COUNT_AT], report[COUNT_AT + 1]])).min(500);
    let keystrokes = report[KEYSTROKES_AT..]
        .chunks_exact(4)
        .take(count)
        .map(|k| Keystroke {
            key: k[0],
            action: k[1],
            period_ms: u16::from_le_bytes([k[2], k[3]]),
        })
        .collect();
    Some(Macro {
        macroset: text_at(&report[NAMES_AT..NAMES_AT + NAME_LEN]),
        name: text_at(&report[NAMES_AT + NAME_LEN..COUNT_AT]),
        keystrokes,
    })
}

/// Plain-language summary of a macro, such as `press 1, release 1`.
pub fn describe_macro(value: &Macro) -> String {
    let steps: Vec<String> = value
        .keystrokes
        .iter()
        .take(6)
        .map(|k| {
            let verb = match k.action {
                1 => "press",
                2 => "release",
                _ => "step",
            };
            format!("{verb} {}", key_name(k.key))
        })
        .collect();
    let more = value.keystrokes.len().saturating_sub(steps.len());
    let suffix = if more > 0 { format!(", and {more} more") } else { String::new() };
    format!("\"{}\": {}{suffix}", value.name, steps.join(", "))
}

/// Builds the "make this the active profile" command.
pub fn actual_profile_report(profile: u8) -> Option<[u8; 3]> {
    (profile < PROFILE_COUNT).then_some([ACTUAL_PROFILE.id, 3, profile])
}

/// Turns a profile number as people count it, 1 to 5, into the keyboard's 0 to 4.
pub fn profile_index(number: &str) -> Option<u8> {
    let number: u8 = number.parse().ok()?;
    (1..=PROFILE_COUNT).contains(&number).then(|| number - 1)
}

/// True when bytes are a complete, correct report for this profile.
pub fn valid_profile_report(report: &Report, profile: u8, bytes: &[u8]) -> bool {
    bytes.len() == report.len
        && bytes[0] == report.id
        && usize::from(bytes[1]) == report.len
        && bytes[2] == profile
        && checksum_ok(bytes)
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
    with_binding(report, group, position, Binding { action: ACTION_SHORTCUT, modifier, key })
}

/// Returns a copy of a key report with one key put back to what it does when
/// unchanged, for groups that have defaults.
pub fn with_default(report: &[u8], group: &KeyGroup, position: usize) -> Option<Vec<u8>> {
    let action = *group.defaults?.get(position)?;
    with_binding(report, group, position, Binding { action, modifier: 0, key: 0 })
}

/// Finds a settable key by name: `T1`, `Shift+T1`, `M1`, `Shift+M1`, `F1`,
/// `Mute`, or an Easy-Shift zone key such as `Shift+Q`.
pub fn find_key(label: &str) -> Option<(&'static KeyGroup, usize)> {
    for group in [&THUMB_KEYS, &MACRO_KEYS, &FUNCTION_KEYS, &MEDIA_KEYS] {
        if let Some(i) = group.labels.iter().position(|l| l.eq_ignore_ascii_case(label)) {
            return Some((group, i));
        }
    }
    let (prefix, key) = label.split_at_checked(6)?;
    if !prefix.eq_ignore_ascii_case("Shift+") {
        return None;
    }
    let i = EASYZONE_KEYS.labels.iter().position(|l| l.eq_ignore_ascii_case(key))?;
    Some((&EASYZONE_KEYS, i))
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

/// Lighting for one profile. Ranges are from roccat-tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Light {
    /// 0 (off) to 5.
    pub brightness: u8,
    /// The brightness, 0 to 5, that the keyboard dims to after the timeout.
    pub dimness: u8,
    /// Minutes without typing before dimming, 1 to 255.
    pub timeout: u8,
}

pub const LIGHT_MAX: u8 = 5;

/// Factory lighting from roccat-tools' default profile.
pub const LIGHT_DEFAULT: Light = Light { brightness: 3, dimness: 1, timeout: 15 };

impl Light {
    pub fn is_valid(&self) -> bool {
        self.brightness <= LIGHT_MAX && self.dimness <= LIGHT_MAX && self.timeout >= 1
    }
}

pub fn parse_light(report: &[u8]) -> Option<Light> {
    if report.len() != LIGHT.len || report[0] != LIGHT.id || !checksum_ok(report) {
        return None;
    }
    Some(Light { brightness: report[5], dimness: report[6], timeout: report[7] })
}

/// Returns a copy of a light report with new values and checksum. Bytes 3 and 4
/// are left as the keyboard had them (roccat-tools always writes 3 and 0).
pub fn with_light(report: &[u8], light: Light) -> Option<Vec<u8>> {
    parse_light(report)?;
    if !light.is_valid() {
        return None;
    }
    let mut updated = report.to_vec();
    updated[5] = light.brightness;
    updated[6] = light.dimness;
    updated[7] = light.timeout;
    let sum = checksum(&updated[..8]);
    updated[8..].copy_from_slice(&sum.to_le_bytes());
    Some(updated)
}

pub fn describe_light(light: Light) -> String {
    format!(
        "brightness {} of {LIGHT_MAX}, dims to {} after {} min",
        light.brightness, light.dimness, light.timeout
    )
}

fn with_binding(report: &[u8], group: &KeyGroup, position: usize, binding: Binding) -> Option<Vec<u8>> {
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

    // Real bytes from dumps/2026-11-05-first-probe.txt.
    const THUMBSTER: [u8; 23] = [
        0x0b, 0x17, 0x00, 0xbe, 0xbe, 0xbe, 0xb9, 0xb9, 0xb9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0x87, 0x04,
    ];
    const CAPSLOCK: [u8; 6] = [0x13, 0x06, 0x00, 0x8c, 0xa5, 0x00];

    // Profile 1 Easy-Shift zone, unchanged keys, from the same dump.
    fn easyzone() -> Vec<u8> {
        let mut report = vec![0x09, 0x41, 0x00];
        report.extend_from_slice(&EASYZONE_DEFAULTS);
        report.extend_from_slice(&[0; 40]);
        report.extend_from_slice(&[0xdc, 0x01]);
        report
    }

    fn macro_report(name: &str, strokes: &[(u8, u8)]) -> Vec<u8> {
        let mut report = vec![0u8; FEATURE_BUFFER_LEN];
        report[0] = MACRO_REPORT_ID;
        report[1..3].copy_from_slice(&0x0823u16.to_le_bytes());
        report[55..55 + name.len()].copy_from_slice(name.as_bytes());
        report[79..81].copy_from_slice(&(strokes.len() as u16).to_le_bytes());
        for (i, (key, action)) in strokes.iter().enumerate() {
            report[81 + i * 4] = *key;
            report[82 + i * 4] = *action;
        }
        let sum = checksum(&report[..FEATURE_BUFFER_LEN - 2]);
        report[FEATURE_BUFFER_LEN - 2..].copy_from_slice(&sum.to_le_bytes());
        report
    }

    #[test]
    fn real_reports_have_valid_checksums() {
        assert!(checksum_ok(&THUMBSTER));
        assert!(checksum_ok(&CAPSLOCK));
    }

    #[test]
    fn a_changed_byte_fails_the_checksum() {
        let mut changed = THUMBSTER;
        changed[3] = 0x1e;
        assert!(!checksum_ok(&changed));
    }

    #[test]
    fn too_short_is_not_valid() {
        assert!(!checksum_ok(&[0x01]));
    }

    #[test]
    fn report_lengths_match_real_reports() {
        assert_eq!(KEYS_THUMBSTER.len, THUMBSTER.len());
        assert_eq!(KEYS_CAPSLOCK.len, CAPSLOCK.len());
    }

    #[test]
    fn key_report_sizes_fit_their_key_counts() {
        for group in [THUMB_KEYS, MACRO_KEYS, EASYZONE_KEYS, FUNCTION_KEYS, MEDIA_KEYS] {
            assert_eq!(group.report.len, 5 + 3 * group.labels.len());
            if let Some(defaults) = group.defaults {
                assert_eq!(defaults.len(), group.labels.len());
            }
        }
    }

    #[test]
    fn control_report_builds_a_known_select() {
        assert_eq!(control_report(1, 0xd0), Some([0x04, 1, 0xd0]));
        assert_eq!(control_report(4, 0x23), Some([0x04, 4, 0x23]));
    }

    #[test]
    fn control_report_refuses_anything_unknown() {
        assert_eq!(control_report(5, 0xd0), None);
        assert_eq!(control_report(0, 0x24), None);
        assert_eq!(control_report(0, 0xff), None);
    }

    #[test]
    fn bindings_split_the_three_arrays() {
        let mut report = THUMBSTER;
        report[3] = ACTION_SHORTCUT;
        report[9] = 0x02;
        report[15] = 0x1e;
        let keys = bindings(&report, 6).unwrap();
        assert_eq!(keys[0], Binding { action: 0xb8, modifier: 0x02, key: 0x1e });
        assert_eq!(keys[5], Binding { action: 0xb9, modifier: 0, key: 0 });
        assert_eq!(bindings(&CAPSLOCK, 6), None);
    }

    #[test]
    fn describe_names_actions() {
        let bind = |action, modifier, key| Binding { action, modifier, key };
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
    fn parse_macro_reads_the_name_and_keystrokes() {
        let parsed = parse_macro(&macro_report("Slot 1", &[(0x1e, 1), (0x1e, 2)])).unwrap();
        assert_eq!(parsed.name, "Slot 1");
        assert_eq!(parsed.keystrokes.len(), 2);
        assert_eq!(describe_macro(&parsed), "\"Slot 1\": press 1, release 1");
    }

    #[test]
    fn parse_macro_rejects_a_bad_checksum() {
        let mut report = macro_report("Slot 1", &[(0x1e, 1)]);
        report[100] ^= 0xff;
        assert_eq!(parse_macro(&report), None);
    }

    #[test]
    fn with_shortcut_changes_one_key_and_keeps_the_report_valid() {
        let updated = with_shortcut(&THUMBSTER, &THUMB_KEYS, 0, 0, 0x24).unwrap();
        assert!(checksum_ok(&updated));
        let keys = bindings(&updated, 6).unwrap();
        assert_eq!(keys[0], Binding { action: ACTION_SHORTCUT, modifier: 0, key: 0x24 });
        assert_eq!(keys[1], bindings(&THUMBSTER, 6).unwrap()[1]);
        assert_eq!(describe(keys[0]), "shortcut 7");
    }

    #[test]
    fn with_shortcut_refuses_a_key_outside_the_group() {
        assert_eq!(with_shortcut(&THUMBSTER, &THUMB_KEYS, 6, 0, 0x24), None);
    }

    #[test]
    fn valid_profile_report_checks_every_field() {
        assert!(valid_profile_report(&KEYS_THUMBSTER, 0, &THUMBSTER));
        assert!(!valid_profile_report(&KEYS_THUMBSTER, 4, &THUMBSTER));
        assert!(!valid_profile_report(&KEYS_MACRO, 0, &THUMBSTER));
        let mut changed = THUMBSTER;
        changed[3] = 0x1e;
        assert!(!valid_profile_report(&KEYS_THUMBSTER, 0, &changed));
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
        for text in ["", "Ctrl+", "Ctrl+Ctrl+1", "Ctrl", "Foo", "F25", "F0", "Hyper+1", "1+Ctrl", "Num10"] {
            assert_eq!(parse_shortcut(text), None, "{text}");
        }
    }

    #[test]
    fn every_named_key_reads_back_as_itself() {
        for code in (0x04..=0x65).chain(0x68..=0x73) {
            let name = key_name(code);
            if !name.starts_with("key ") {
                assert_eq!(key_code(&name), Some(code), "{name}");
            }
        }
        assert_eq!(shortcut_text(0b1100, 0x1d), "Ctrl+Alt+Z");
        assert_eq!(parse_shortcut(&shortcut_text(0b11110, 0x3e)), Some((0b11110, 0x3e)));
    }

    #[test]
    fn actual_profile_report_refuses_a_missing_profile() {
        assert_eq!(actual_profile_report(4), Some([0x05, 3, 4]));
        assert_eq!(actual_profile_report(5), None);
    }

    #[test]
    fn profile_index_counts_from_one() {
        assert_eq!(profile_index("1"), Some(0));
        assert_eq!(profile_index("5"), Some(4));
        assert_eq!(profile_index("0"), None);
        assert_eq!(profile_index("6"), None);
        assert_eq!(profile_index("x"), None);
    }

    #[test]
    fn easyzone_sample_matches_the_dump() {
        assert!(valid_profile_report(&KEYS_EASYZONE, 0, &easyzone()));
    }

    #[test]
    fn easyzone_default_undoes_a_shortcut() {
        let changed = with_shortcut(&easyzone(), &EASYZONE_KEYS, 5, 0b0100, 0x1e).unwrap();
        assert!(checksum_ok(&changed));
        assert_eq!(describe(bindings(&changed, 20).unwrap()[5]), "shortcut Ctrl+1");
        assert_eq!(with_default(&changed, &EASYZONE_KEYS, 5).unwrap(), easyzone());
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
        assert_eq!(with_all_defaults(&changed, &EASYZONE_KEYS).unwrap(), easyzone());
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
        assert_eq!(with_capslock(&from_8c, CAPSLOCK_DEFAULT).unwrap(), profile_5);
        assert!(valid_profile_report(&KEYS_CAPSLOCK, 4, &profile_5));
        assert_eq!(with_capslock(&THUMBSTER, CAPSLOCK_DEFAULT), None);
    }

    // Profile 1 light report from dumps/2026-11-05-first-probe.txt.
    const LIGHT_REPORT: [u8; 10] = [0x10, 0x0a, 0x00, 0x03, 0x00, 0x05, 0x01, 0x0f, 0x32, 0x00];

    #[test]
    fn parse_light_reads_the_real_report() {
        let light = parse_light(&LIGHT_REPORT).unwrap();
        assert_eq!(light, Light { brightness: 5, dimness: 1, timeout: 15 });
        assert_eq!(describe_light(light), "brightness 5 of 5, dims to 1 after 15 min");
        assert!(valid_profile_report(&LIGHT, 0, &LIGHT_REPORT));
    }

    #[test]
    fn with_light_changes_values_and_keeps_the_report_valid() {
        let updated = with_light(&LIGHT_REPORT, Light { brightness: 2, ..LIGHT_DEFAULT }).unwrap();
        assert!(valid_profile_report(&LIGHT, 0, &updated));
        assert_eq!(parse_light(&updated).unwrap().brightness, 2);
        assert_eq!(updated[3..5], LIGHT_REPORT[3..5]);
        let back = with_light(&updated, Light { brightness: 5, dimness: 1, timeout: 15 }).unwrap();
        assert_eq!(back, LIGHT_REPORT);
    }

    #[test]
    fn with_light_refuses_out_of_range_values() {
        for light in [
            Light { brightness: 6, ..LIGHT_DEFAULT },
            Light { dimness: 6, ..LIGHT_DEFAULT },
            Light { timeout: 0, ..LIGHT_DEFAULT },
        ] {
            assert_eq!(with_light(&LIGHT_REPORT, light), None);
        }
        assert_eq!(with_light(&CAPSLOCK, LIGHT_DEFAULT), None);
    }
}

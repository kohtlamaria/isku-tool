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
    /// The first key's macro index, for groups that can hold macros.
    pub first_macro_index: u8,
}

pub const THUMB_KEYS: KeyGroup = KeyGroup {
    title: "Thumb keys",
    report: KEYS_THUMBSTER,
    labels: &["T1", "T2", "T3", "Shift+T1", "Shift+T2", "Shift+T3"],
    first_macro_index: 0x1e,
};

pub const MACRO_KEYS: KeyGroup = KeyGroup {
    title: "Macro keys",
    report: KEYS_MACRO,
    labels: &["M1", "M2", "M3", "M4", "M5", "Shift+M1", "Shift+M2", "Shift+M3", "Shift+M4", "Shift+M5"],
    first_macro_index: 0x00,
};

pub const EASYZONE_KEYS: KeyGroup = KeyGroup {
    title: "Easy-Shift zone",
    report: KEYS_EASYZONE,
    labels: &[
        "1", "2", "3", "4", "5", "Q", "W", "E", "R", "T", "A", "S", "D", "F", "G", "Z", "X", "C",
        "V", "B",
    ],
    first_macro_index: 0x0a,
};

/// What the Easy-Shift zone holds when a key has not been changed.
pub const EASYZONE_DEFAULTS: [u8; 20] = [
    0x1e, 0x1f, 0x20, 0x21, 0x22, 0x14, 0x1a, 0x08, 0x15, 0x17, 0x04, 0x16, 0x07, 0x09, 0x0a,
    0x1d, 0x1b, 0x06, 0x19, 0x05,
];

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
pub fn key_name(code: u8) -> String {
    match code {
        0x04..=0x1d => char::from(b'A' + (code - 0x04)).to_string(),
        0x1e..=0x26 => (code - 0x1d).to_string(),
        0x27 => "0".to_string(),
        0x28 => "Enter".to_string(),
        0x29 => "Esc".to_string(),
        0x2a => "Backspace".to_string(),
        0x2b => "Tab".to_string(),
        0x2c => "Space".to_string(),
        0x3a..=0x45 => format!("F{}", code - 0x39),
        _ => format!("key 0x{code:02x}"),
    }
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
        let mut text = String::new();
        for (bit, name) in [(2, "Ctrl+"), (1, "Shift+"), (3, "Alt+"), (4, "Win+")] {
            if binding.modifier & (1 << bit) != 0 {
                text.push_str(name);
            }
        }
        return format!("shortcut {text}{}", key_name(binding.key));
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

#[cfg(test)]
mod tests {
    use super::*;

    // Real bytes from dumps/2026-11-05-first-probe.txt.
    const THUMBSTER: [u8; 23] = [
        0x0b, 0x17, 0x00, 0xbe, 0xbe, 0xbe, 0xb9, 0xb9, 0xb9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0x87, 0x04,
    ];
    const CAPSLOCK: [u8; 6] = [0x13, 0x06, 0x00, 0x8c, 0xa5, 0x00];

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
        for group in [THUMB_KEYS, MACRO_KEYS, EASYZONE_KEYS] {
            assert_eq!(group.report.len, 5 + 3 * group.labels.len());
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
}

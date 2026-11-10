//! Reading and building macro reports.

use super::*;

/// Where each field of a macro report starts. See docs/protocol.md.
const SIZE_AT: usize = 1;
const PROFILE_AT: usize = 3;
const KEY_AT: usize = 4;
const HEADER_AT: usize = 5;
const NAMES_AT: usize = 31;
const NAME_LEN: usize = 24;
const COUNT_AT: usize = NAMES_AT + 2 * NAME_LEN;
const KEYSTROKES_AT: usize = COUNT_AT + 2;
const CHECKSUM_AT: usize = FEATURE_BUFFER_LEN - 2;
/// Where the type and loop count sit in `Macro::header` (bytes 5 and 6 of
/// the report).
const TYPE_IN_HEADER: usize = 0;
const LOOP_IN_HEADER: usize = 1;

/// A macro holds at most this many keystrokes.
pub const MAX_KEYSTROKES: usize = (CHECKSUM_AT - KEYSTROKES_AT) / 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keystroke {
    pub key: u8,
    pub action: u8,
    pub period_ms: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Macro {
    /// Bytes 5 to 30 as the keyboard sent them: the type (0xbe), the loop
    /// count, and a field roccat-tools does not explain, which once held
    /// "ISKU". Kept as they are, so a rebuilt macro matches the original.
    pub header: [u8; NAMES_AT - HEADER_AT],
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
    if report.len() != FEATURE_BUFFER_LEN || report[0] != MACRO_REPORT_ID || !checksum_ok(report) {
        return None;
    }
    let count = usize::from(u16::from_le_bytes([report[COUNT_AT], report[COUNT_AT + 1]]))
        .min(MAX_KEYSTROKES);
    let keystrokes = report[KEYSTROKES_AT..CHECKSUM_AT]
        .as_chunks::<4>()
        .0
        .iter()
        .take(count)
        .map(|k| Keystroke {
            key: k[0],
            action: k[1],
            period_ms: u16::from_le_bytes([k[2], k[3]]),
        })
        .collect();
    Some(Macro {
        header: report[HEADER_AT..NAMES_AT].try_into().ok()?,
        macroset: text_at(&report[NAMES_AT..NAMES_AT + NAME_LEN]),
        name: text_at(&report[NAMES_AT + NAME_LEN..COUNT_AT]),
        keystrokes,
    })
}

/// Writes a name into its 24-byte field. A name that fills the field gets no
/// closing zero, as the keyboard stores it. Only plain ASCII is accepted.
fn put_text(field: &mut [u8], text: &str) -> Option<()> {
    if !text.is_ascii() || text.contains('\0') || text.len() > field.len() {
        return None;
    }
    field[..text.len()].copy_from_slice(text.as_bytes());
    Some(())
}

/// Builds the complete macro report for one profile and macro slot, with
/// its size and checksum. Returns None for a slot that does not exist, too
/// many keystrokes, or a name that does not fit.
pub fn build_macro(profile: u8, key_index: u8, value: &Macro) -> Option<Vec<u8>> {
    if profile >= PROFILE_COUNT
        || key_index > LAST_MACRO_KEY_INDEX
        || value.keystrokes.len() > MAX_KEYSTROKES
    {
        return None;
    }
    let mut report = vec![0u8; FEATURE_BUFFER_LEN];
    report[0] = MACRO_REPORT_ID;
    report[SIZE_AT..SIZE_AT + 2].copy_from_slice(&(FEATURE_BUFFER_LEN as u16).to_le_bytes());
    report[PROFILE_AT] = profile;
    report[KEY_AT] = key_index;
    report[HEADER_AT..NAMES_AT].copy_from_slice(&value.header);
    put_text(&mut report[NAMES_AT..NAMES_AT + NAME_LEN], &value.macroset)?;
    put_text(&mut report[NAMES_AT + NAME_LEN..COUNT_AT], &value.name)?;
    report[COUNT_AT..KEYSTROKES_AT].copy_from_slice(&(value.keystrokes.len() as u16).to_le_bytes());
    for (k, slot) in value.keystrokes.iter().zip(
        report[KEYSTROKES_AT..CHECKSUM_AT]
            .as_chunks_mut::<4>()
            .0
            .iter_mut(),
    ) {
        slot[0] = k.key;
        slot[1] = k.action;
        slot[2..].copy_from_slice(&k.period_ms.to_le_bytes());
    }
    let sum = checksum(&report[..CHECKSUM_AT]);
    report[CHECKSUM_AT..].copy_from_slice(&sum.to_le_bytes());
    Some(report)
}

/// True when bytes are a complete, correct macro report for this profile
/// and slot, holding no more keystrokes than fit.
pub fn valid_macro_report(profile: u8, key_index: u8, bytes: &[u8]) -> bool {
    let field = |at: usize| usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]));
    bytes.len() == FEATURE_BUFFER_LEN
        && bytes[0] == MACRO_REPORT_ID
        && field(SIZE_AT) == FEATURE_BUFFER_LEN
        && bytes[PROFILE_AT] == profile
        && bytes[KEY_AT] == key_index
        && field(COUNT_AT) <= MAX_KEYSTROKES
        && checksum_ok(bytes)
}

/// A macro with no keystrokes and no names, which plays nothing. `undo`
/// writes it to a slot that held no macro, since a slot cannot be emptied.
pub fn empty_macro_report(profile: u8, key_index: u8) -> Option<Vec<u8>> {
    let nothing = Macro {
        header: NEW_MACRO_HEADER,
        macroset: String::new(),
        name: String::new(),
        keystrokes: Vec::new(),
    };
    build_macro(profile, key_index, &nothing)
}

/// Returns a copy of a valid macro moved to another profile's same slot: the
/// profile byte is changed and the checksum recomputed.
pub fn macro_for_profile(from: u8, key_index: u8, bytes: &[u8], to: u8) -> Option<Vec<u8>> {
    if to >= PROFILE_COUNT || !valid_macro_report(from, key_index, bytes) {
        return None;
    }
    let mut moved = bytes.to_vec();
    moved[PROFILE_AT] = to;
    let sum = checksum(&moved[..CHECKSUM_AT]);
    moved[CHECKSUM_AT..].copy_from_slice(&sum.to_le_bytes());
    Some(moved)
}

/// The key that plays a macro slot, named as people know it, such as `T1`
/// or `Shift+Q`.
pub fn slot_key_label(key_index: u8) -> Option<String> {
    [&THUMB_KEYS, &MACRO_KEYS, &EASYZONE_KEYS]
        .into_iter()
        .flat_map(|group| (0..group.labels.len()).map(move |position| (group, position)))
        .find(|(group, position)| macro_slot(group, *position) == Some(key_index))
        .map(|(group, position)| key_label(group, position))
}

/// The header for a new macro in a slot: the old macro's, so the field
/// roccat-tools does not explain stays as it was, but always of the macro
/// type and played once, so the macro does what its text says. A slot with
/// no macro gets NEW_MACRO_HEADER.
pub fn header_for_new_macro(old: Option<&Macro>) -> [u8; NAMES_AT - HEADER_AT] {
    let mut header = old.map_or(NEW_MACRO_HEADER, |old| old.header);
    let set = TYPE_IN_HEADER..=LOOP_IN_HEADER;
    header[set.clone()].copy_from_slice(&NEW_MACRO_HEADER[set]);
    header
}

/// The macro report for macro text written into a slot that holds `old`
/// now: its header as `header_for_new_macro` gives it, the tool's macroset
/// name, and the text as its name. None when it does not fit.
pub fn macro_from_text(
    profile: u8,
    key_index: u8,
    old: Option<&Macro>,
    text: &str,
    keystrokes: Vec<Keystroke>,
) -> Option<Vec<u8>> {
    let value = Macro {
        header: header_for_new_macro(old),
        macroset: MACROSET_NAME.to_string(),
        name: macro_name(text),
        keystrokes,
    };
    build_macro(profile, key_index, &value)
}

/// True when a macro has the type and loop count the tool writes, so it
/// plays once and does just what its keystrokes say.
pub fn plays_once(value: &Macro) -> bool {
    header_for_new_macro(Some(value)) == value.header
}

/// Plain-language summary of a macro, such as `press 1, release 1`.
pub fn describe_macro(value: &Macro) -> String {
    if value.keystrokes.is_empty() {
        return "with no keystrokes, plays nothing".to_string();
    }
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
    let suffix = if more > 0 {
        format!(", and {more} more")
    } else {
        String::new()
    };
    format!("\"{}\": {}{suffix}", value.name, steps.join(", "))
}

pub const KEYSTROKE_PRESS: u8 = 1;
pub const KEYSTROKE_RELEASE: u8 = 2;

/// The macroset name the tool gives macros it writes.
pub const MACROSET_NAME: &str = "isku-tool";

/// Byte 5 of every macro the old software saved. It equals the key action
/// for macros, but it is a separate field of the macro report.
const MACRO_TYPE: u8 = 0xbe;

/// Header bytes for a macro written into a slot that held none: the macro
/// type and play once, as in every macro the old software saved.
pub const NEW_MACRO_HEADER: [u8; NAMES_AT - HEADER_AT] = {
    let mut header = [0; NAMES_AT - HEADER_AT];
    header[TYPE_IN_HEADER] = MACRO_TYPE;
    header[LOOP_IN_HEADER] = 1;
    header
};

/// The milliseconds in a step like `wait 50`, ignoring case. None when the
/// step is not a wait at all.
fn wait_step(step: &str) -> Option<&str> {
    let (word, rest) = step.split_at_checked(5)?;
    word.eq_ignore_ascii_case("wait ").then_some(rest)
}

/// Reads macro text such as `Ctrl+C, wait 50, Ctrl+V` into keystrokes. Each
/// step is a key or shortcut, which is pressed and released, modifiers
/// first. `wait N` waits N milliseconds after the step before it.
pub fn parse_macro_text(text: &str) -> Option<Vec<Keystroke>> {
    let mut keystrokes: Vec<Keystroke> = Vec::new();
    let stroke = |key, action| Keystroke {
        key,
        action,
        period_ms: 0,
    };
    for step in text.split(',').map(str::trim) {
        if let Some(ms) = wait_step(step) {
            let ms: u16 = ms.trim().parse().ok().filter(|ms| *ms > 0)?;
            let last = keystrokes.last_mut()?;
            last.period_ms = last.period_ms.checked_add(ms)?;
            continue;
        }
        let (modifier, key) = parse_shortcut(step)?;
        let held: Vec<u8> = MODIFIERS
            .iter()
            .filter(|(bit, _, _)| modifier & (1 << bit) != 0)
            .map(|(_, _, code)| *code)
            .collect();
        keystrokes.extend(held.iter().map(|&code| stroke(code, KEYSTROKE_PRESS)));
        keystrokes.push(stroke(key, KEYSTROKE_PRESS));
        keystrokes.push(stroke(key, KEYSTROKE_RELEASE));
        keystrokes.extend(
            held.iter()
                .rev()
                .map(|&code| stroke(code, KEYSTROKE_RELEASE)),
        );
    }
    (keystrokes.len() <= MAX_KEYSTROKES).then_some(keystrokes)
}

/// Writes keystrokes as macro text that `parse_macro_text` reads back into
/// exactly the same keystrokes, or None when that is not possible, for
/// example an empty macro or one that holds a key down across steps.
pub fn macro_to_text(keystrokes: &[Keystroke]) -> Option<String> {
    let mut steps = Vec::new();
    let mut rest = keystrokes;
    while !rest.is_empty() {
        // A step presses its modifiers and key, then releases them all, so
        // it is twice as long as its run of presses. The key comes last.
        let presses = rest
            .iter()
            .take_while(|k| k.action == KEYSTROKE_PRESS)
            .count();
        let (step, after) = rest.split_at_checked(2 * presses)?;
        let (key, held) = step[..presses].split_last()?;
        let mut modifier = 0u8;
        for k in held {
            let (bit, _, _) = MODIFIERS.iter().find(|(_, _, code)| *code == k.key)?;
            modifier |= 1 << bit;
        }
        steps.push(shortcut_text(modifier, key.key));
        let wait = step.last()?.period_ms;
        if wait > 0 {
            steps.push(format!("wait {wait}"));
        }
        rest = after;
    }
    // Reading the text back catches everything the loop above lets through,
    // such as releases in another order or a wait inside a step.
    let text = steps.join(", ");
    (!text.is_empty() && parse_macro_text(&text)? == keystrokes).then_some(text)
}

/// The name a macro gets from its text: the text, cut to fit the field.
pub fn macro_name(text: &str) -> String {
    text.chars().filter(char::is_ascii).take(NAME_LEN).collect()
}

/// The file name of one macro in a backup or change folder.
pub fn macro_file_name(profile: u8, key_index: u8) -> String {
    format!("profile-{}-macro-{key_index:02x}.bin", profile + 1)
}

/// The file a change folder holds when a slot had no macro before.
pub fn empty_macro_file_name(profile: u8, key_index: u8) -> String {
    format!("profile-{}-macro-{key_index:02x}.empty", profile + 1)
}

/// Reads a macro file name like `profile-5-macro-1e.bin` into its profile
/// (counted from 0), key index, and whether it marks an empty slot
/// (`.empty`).
pub fn parse_macro_file_name(name: &str) -> Option<(u8, u8, bool)> {
    let rest = name.strip_prefix("profile-")?;
    let (rest, empty) = match rest.strip_suffix(".bin") {
        Some(rest) => (rest, false),
        None => (rest.strip_suffix(".empty")?, true),
    };
    let (profile, slot) = rest.split_once("-macro-")?;
    let profile = profile_index(profile)?;
    if slot.len() != 2 || !slot.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let key_index = u8::from_str_radix(slot, 16).ok()?;
    // Only the exact names the tool writes, so one slot has one spelling.
    let canonical = if empty {
        empty_macro_file_name(profile, key_index)
    } else {
        macro_file_name(profile, key_index)
    };
    (key_index <= LAST_MACRO_KEY_INDEX && name == canonical).then_some((profile, key_index, empty))
}

/// The macro slot a key plays, if it has one.
pub fn macro_slot(group: &KeyGroup, position: usize) -> Option<u8> {
    let index = usize::from(group.first_macro_index) + position;
    (position < group.labels.len() && index <= usize::from(LAST_MACRO_KEY_INDEX))
        .then_some(index as u8)
}

/// Returns a copy of a key report with one key set to play its macro.
pub fn with_macro(report: &[u8], group: &KeyGroup, position: usize) -> Option<Vec<u8>> {
    macro_slot(group, position)?;
    with_binding(
        report,
        group,
        position,
        Binding {
            action: ACTION_MACRO,
            modifier: 0,
            key: 0,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::test_data::*;

    #[test]
    fn parse_macro_reads_the_name_and_keystrokes() {
        let parsed = parse_macro(&sample_macro("Slot 1", &[(0x1e, 1), (0x1e, 2)])).unwrap();
        assert_eq!(parsed.name, "Slot 1");
        assert_eq!(parsed.keystrokes.len(), 2);
        assert_eq!(describe_macro(&parsed), "\"Slot 1\": press 1, release 1");
    }

    #[test]
    fn parse_macro_rejects_a_bad_checksum() {
        let mut report = sample_macro("Slot 1", &[(0x1e, 1)]);
        report[100] ^= 0xff;
        assert_eq!(parse_macro(&report), None);
    }

    #[test]
    fn build_macro_rebuilds_a_parsed_macro() {
        let mut report = sample_macro("Slot 1", &[(0x1e, 1), (0x1e, 2)]);
        report[PROFILE_AT] = 4;
        report[KEY_AT] = 0x1e;
        report[HEADER_AT..HEADER_AT + 6].copy_from_slice(&[0xbe, 1, b'I', b'S', b'K', b'U']);
        report[NAMES_AT..NAMES_AT + NAME_LEN].copy_from_slice(b"Star wars force unleashe");
        let sum = checksum(&report[..CHECKSUM_AT]);
        report[CHECKSUM_AT..].copy_from_slice(&sum.to_le_bytes());
        assert!(valid_macro_report(4, 0x1e, &report));

        let parsed = parse_macro(&report).unwrap();
        assert_eq!(parsed.macroset, "Star wars force unleashe");
        assert_eq!(build_macro(4, 0x1e, &parsed).unwrap(), report);
        assert!(!valid_macro_report(3, 0x1e, &report));
        assert!(!valid_macro_report(4, 0x1f, &report));
    }

    #[test]
    fn empty_macros_and_new_headers() {
        let empty = empty_macro_report(4, 0x00).unwrap();
        assert!(valid_macro_report(4, 0x00, &empty));
        let parsed = parse_macro(&empty).unwrap();
        assert_eq!(parsed.keystrokes, []);
        assert_eq!(describe_macro(&parsed), "with no keystrokes, plays nothing");

        let mut old = parse_macro(&sample_macro("Old", &[(0x1e, 1)])).unwrap();
        old.header[..6].copy_from_slice(&[0, 5, b'I', b'S', b'K', b'U']);
        let header = header_for_new_macro(Some(&old));
        assert_eq!(header[..6], [MACRO_TYPE, 1, b'I', b'S', b'K', b'U']);
        assert_eq!(header_for_new_macro(None), NEW_MACRO_HEADER);
    }

    #[test]
    fn macro_for_profile_moves_a_macro_and_keeps_it_valid() {
        let mut report = sample_macro("Slot 1", &[(0x1e, 1), (0x1e, 2)]);
        report[KEY_AT] = 0x1e;
        let sum = checksum(&report[..CHECKSUM_AT]);
        report[CHECKSUM_AT..].copy_from_slice(&sum.to_le_bytes());

        let moved = macro_for_profile(0, 0x1e, &report, 4).unwrap();
        assert!(valid_macro_report(4, 0x1e, &moved));
        assert_eq!(parse_macro(&moved), parse_macro(&report));
        assert_eq!(macro_for_profile(4, 0x1e, &moved, 0).unwrap(), report);
        // Wrong source profile or slot, target out of range.
        assert_eq!(macro_for_profile(1, 0x1e, &report, 4), None);
        assert_eq!(macro_for_profile(0, 0x1f, &report, 4), None);
        assert_eq!(macro_for_profile(0, 0x1e, &report, 5), None);
    }

    #[test]
    fn slots_are_named_by_their_key() {
        assert_eq!(slot_key_label(0x00).as_deref(), Some("M1"));
        assert_eq!(slot_key_label(0x09).as_deref(), Some("Shift+M5"));
        assert_eq!(slot_key_label(0x0a).as_deref(), Some("Shift+1"));
        assert_eq!(slot_key_label(0x1e).as_deref(), Some("T1"));
        assert_eq!(slot_key_label(0x23).as_deref(), Some("Shift+T3"));
        assert_eq!(slot_key_label(0x24), None);
    }

    #[test]
    fn too_many_keystrokes_is_not_valid() {
        let mut report = empty_macro_report(0, 0).unwrap();
        report[COUNT_AT..KEYSTROKES_AT].copy_from_slice(&0xffffu16.to_le_bytes());
        let sum = checksum(&report[..CHECKSUM_AT]);
        report[CHECKSUM_AT..].copy_from_slice(&sum.to_le_bytes());
        assert!(!valid_macro_report(0, 0, &report));
    }

    #[test]
    fn build_macro_refuses_what_does_not_fit() {
        let good = parse_macro(&sample_macro("Slot 1", &[(0x1e, 1)])).unwrap();
        assert!(build_macro(4, LAST_MACRO_KEY_INDEX, &good).is_some());
        assert!(build_macro(5, 0, &good).is_none());
        assert!(build_macro(0, LAST_MACRO_KEY_INDEX + 1, &good).is_none());
        let long_name = Macro {
            name: "x".repeat(NAME_LEN + 1),
            ..good.clone()
        };
        assert!(build_macro(0, 0, &long_name).is_none());
        let not_ascii = Macro {
            name: "Jõud".to_string(),
            ..good.clone()
        };
        assert!(build_macro(0, 0, &not_ascii).is_none());
        let too_many = Macro {
            keystrokes: vec![good.keystrokes[0]; MAX_KEYSTROKES + 1],
            ..good
        };
        assert!(build_macro(0, 0, &too_many).is_none());
    }

    fn strokes(text: &str) -> Vec<(u8, u8, u16)> {
        parse_macro_text(text)
            .unwrap()
            .iter()
            .map(|k| (k.key, k.action, k.period_ms))
            .collect()
    }

    #[test]
    fn macro_text_presses_modifiers_around_the_key() {
        assert_eq!(strokes("7"), [(0x24, 1, 0), (0x24, 2, 0)]);
        assert_eq!(
            strokes("Win+Ctrl+C"),
            [
                (0xe0, 1, 0),
                (0xe3, 1, 0),
                (0x06, 1, 0),
                (0x06, 2, 0),
                (0xe3, 2, 0),
                (0xe0, 2, 0)
            ]
        );
        // The same as the "Ctrl q" macro the old software saved.
        let ctrl_q = strokes("Ctrl+Q");
        assert_eq!(
            ctrl_q.iter().map(|s| (s.0, s.1)).collect::<Vec<_>>(),
            [(0xe0, 1), (0x14, 1), (0x14, 2), (0xe0, 2)]
        );
    }

    #[test]
    fn macro_text_waits_after_the_step_before() {
        assert_eq!(
            strokes("1, wait 50, WAIT 10 ,2"),
            [(0x1e, 1, 0), (0x1e, 2, 60), (0x1f, 1, 0), (0x1f, 2, 0)]
        );
        for bad in [
            "",
            "wait 50, 1",
            "1, wait 0",
            "1, wait x",
            "1,,2",
            "Ctrl+",
            "Hyper+1",
        ] {
            assert_eq!(parse_macro_text(bad), None, "{bad}");
        }
        assert!(parse_macro_text(&vec!["1"; MAX_KEYSTROKES / 2].join(",")).is_some());
        assert!(parse_macro_text(&vec!["1"; MAX_KEYSTROKES / 2 + 1].join(",")).is_none());
    }

    #[test]
    fn macro_to_text_reads_back_the_same() {
        for (text, expected) in [
            ("7", "7"),
            ("ctrl+c, wait 50, Ctrl+V", "Ctrl+C, wait 50, Ctrl+V"),
            ("Win+Ctrl+C", "Ctrl+Win+C"),
            ("1, wait 50, wait 10, 2", "1, wait 60, 2"),
            ("LeftCtrl, Ctrl+LeftShift", "LeftCtrl, Ctrl+LeftShift"),
        ] {
            let keystrokes = parse_macro_text(text).unwrap();
            assert_eq!(macro_to_text(&keystrokes).as_deref(), Some(expected));
            assert_eq!(parse_macro_text(expected).unwrap(), keystrokes);
        }
    }

    #[test]
    fn macro_to_text_refuses_what_text_cannot_say() {
        let k = |key, action, period_ms| Keystroke {
            key,
            action,
            period_ms,
        };
        for keystrokes in [
            // Empty.
            vec![],
            // Shift held down across two keys.
            vec![
                k(0xe1, 1, 0),
                k(0x04, 1, 0),
                k(0x04, 2, 0),
                k(0x05, 1, 0),
                k(0x05, 2, 0),
                k(0xe1, 2, 0),
            ],
            // Released in the wrong order.
            vec![k(0xe0, 1, 0), k(0x06, 1, 0), k(0xe0, 2, 0), k(0x06, 2, 0)],
            // A wait between press and release.
            vec![k(0x1e, 1, 30), k(0x1e, 2, 0)],
            // Only a press, or an unknown action.
            vec![k(0x1e, 1, 0)],
            vec![k(0x1e, 3, 0), k(0x1e, 2, 0)],
            // A key with no name.
            vec![k(0xf0, 1, 0), k(0xf0, 2, 0)],
        ] {
            assert_eq!(macro_to_text(&keystrokes), None, "{keystrokes:?}");
        }
    }

    #[test]
    fn macro_names_fit_the_field() {
        assert_eq!(macro_name("Ctrl+C"), "Ctrl+C");
        assert_eq!(macro_name(&"x".repeat(30)).len(), NAME_LEN);
    }

    #[test]
    fn macro_file_names_read_back() {
        assert_eq!(macro_file_name(4, 0x1e), "profile-5-macro-1e.bin");
        assert_eq!(
            parse_macro_file_name(&macro_file_name(4, 0x1e)),
            Some((4, 0x1e, false))
        );
        assert_eq!(
            parse_macro_file_name(&empty_macro_file_name(0, 0x00)),
            Some((0, 0x00, true))
        );
        for bad in [
            "profile-5-macro-24.bin",
            "profile-6-macro-1e.bin",
            "profile-5-macro-1.bin",
            "profile-5-macro-1g.bin",
            "profile-5-macro-1E.bin",
            "profile-05-macro-1e.bin",
            "profile-5-light.bin",
            "profile-5-macro-1e.bin.tmp",
        ] {
            assert_eq!(parse_macro_file_name(bad), None, "{bad}");
        }
    }

    #[test]
    fn only_macro_capable_keys_get_a_macro() {
        assert_eq!(macro_slot(&THUMB_KEYS, 0), Some(0x1e));
        assert_eq!(macro_slot(&THUMB_KEYS, 5), Some(0x23));
        assert_eq!(macro_slot(&MACRO_KEYS, 9), Some(0x09));
        assert_eq!(macro_slot(&EASYZONE_KEYS, 19), Some(0x1d));
        assert_eq!(macro_slot(&FUNCTION_KEYS, 0), None);
        assert_eq!(macro_slot(&THUMB_KEYS, 6), None);
        let updated = with_macro(&THUMBSTER, &THUMB_KEYS, 3).unwrap();
        assert!(checksum_ok(&updated));
        assert_eq!(bindings(&updated, 6).unwrap()[3].action, ACTION_MACRO);
        assert_eq!(with_macro(&THUMBSTER, &FUNCTION_KEYS, 0), None);
    }

    #[test]
    fn names_cover_modifier_keys_and_app_actions() {
        assert_eq!(key_name(0xe0), "LeftCtrl");
        assert_eq!(key_name(0xe7), "RightWin");
        let bind = |action| Binding {
            action,
            modifier: 0,
            key: 0,
        };
        assert_eq!(describe(bind(0xc3)), "open browser (0xc3)");
        assert_eq!(describe(bind(0xe9)), "unknown action (0xe9)");
        let ctrl_q = Macro {
            header: [0; NAMES_AT - HEADER_AT],
            macroset: String::new(),
            name: "Ctrl q".to_string(),
            keystrokes: [(0xe0, 1), (0x14, 1), (0x14, 2), (0xe0, 2)]
                .iter()
                .map(|&(key, action)| Keystroke {
                    key,
                    action,
                    period_ms: 0,
                })
                .collect(),
        };
        assert_eq!(
            describe_macro(&ctrl_q),
            "\"Ctrl q\": press LeftCtrl, press Q, release Q, release LeftCtrl"
        );
    }
}

//! Reading macro reports.

use super::*;

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
    let suffix = if more > 0 {
        format!(", and {more} more")
    } else {
        String::new()
    };
    format!("\"{}\": {}{suffix}", value.name, steps.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::test_data::*;

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

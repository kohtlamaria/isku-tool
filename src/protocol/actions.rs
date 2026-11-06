//! Special actions such as profile switching.

use super::*;

/// Special actions `set` accepts by name. Codes are from roccat-tools.
///
/// Tested on our keyboard: profile switching works on any key. Media actions
/// do not: the 0xa0 codes only work on the media key row, and the 0xc0-0xd2
/// codes did nothing without Roccat's software, so neither is offered.
pub const ACTIONS: &[(&str, u8)] = &[
    ("off", 0x00),
    ("profile-up", 0xb0),
    ("profile-down", 0xb1),
    ("profile-cycle", 0xb2),
    ("profile-1", 0xb3),
    ("profile-2", 0xb4),
    ("profile-3", 0xb5),
    ("profile-4", 0xb6),
    ("profile-5", 0xb7),
    ("easyshift", 0xff),
];

pub fn action_code(name: &str) -> Option<u8> {
    ACTIONS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, code)| *code)
}

/// Returns a copy of a key report with one key set to a special action.
pub fn with_action(
    report: &[u8],
    group: &KeyGroup,
    position: usize,
    action: u8,
) -> Option<Vec<u8>> {
    if !ACTIONS.iter().any(|(_, code)| *code == action) {
        return None;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::test_data::*;

    #[test]
    fn actions_have_unique_names_that_are_not_key_names() {
        for (i, (name, code)) in ACTIONS.iter().enumerate() {
            assert_eq!(action_code(name), Some(*code));
            assert_eq!(key_code(name), None, "{name} is also a key name");
            assert!(ACTIONS[i + 1..].iter().all(|(n, _)| n != name));
        }
        assert_eq!(action_code("Profile-Cycle"), Some(0xb2));
        assert_eq!(action_code("shutdown"), None);
    }

    #[test]
    fn with_action_sets_the_action_and_clears_the_shortcut() {
        let shortcut = with_shortcut(&THUMBSTER, &THUMB_KEYS, 2, 0b0100, 0x1e).unwrap();
        let updated = with_action(&shortcut, &THUMB_KEYS, 2, 0xb2).unwrap();
        assert!(checksum_ok(&updated));
        assert_eq!(
            bindings(&updated, 6).unwrap()[2],
            Binding {
                action: 0xb2,
                modifier: 0,
                key: 0
            }
        );
        assert_eq!(
            describe(bindings(&updated, 6).unwrap()[2]),
            "profile cycle (0xb2)"
        );
        assert_eq!(with_action(&THUMBSTER, &THUMB_KEYS, 2, 0x81), None);
    }
}

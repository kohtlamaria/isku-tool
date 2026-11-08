//! The Isku report format. No USB code here, so all of this can be unit tested.
//! Constants come from the Linux driver and roccat-tools. See docs/protocol.md.
//!
//! This file holds the report table, checksums and profile helpers. Key
//! bindings, special actions, lighting and macros each have their own module,
//! and everything is re-exported here so callers use `protocol::...`.

mod actions;
mod keys;
mod light;
mod macros;
#[cfg(test)]
mod test_data;

pub use actions::*;
pub use keys::*;
pub use light::*;
pub use macros::*;

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
/// Seen when selecting a macro slot that holds no macro.
pub const STATUS_EMPTY: u8 = 4;

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

const fn report(
    id: u8,
    len: usize,
    name: &'static str,
    request: Option<u8>,
    has_checksum: bool,
) -> Report {
    Report {
        id,
        len,
        name,
        request,
        has_checksum,
    }
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
    KEY_MASK,
    KEYS_FUNCTION,
    LIGHT,
    KEYS_CAPSLOCK,
    KEYS_MACRO,
    KEYS_EASYZONE,
    KEYS_MEDIA,
    KEYS_THUMBSTER,
];

/// The 16-bit sum of every byte, which the keyboard stores little-endian.
pub fn checksum(bytes: &[u8]) -> u16 {
    bytes
        .iter()
        .fold(0u16, |sum, b| sum.wrapping_add(u16::from(*b)))
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

/// Returns a copy of a valid profile report moved to another profile: the
/// profile byte is changed and the checksum recomputed.
pub fn for_profile(report: &Report, from: u8, bytes: &[u8], to: u8) -> Option<Vec<u8>> {
    if !report.has_checksum || to >= PROFILE_COUNT || !valid_profile_report(report, from, bytes) {
        return None;
    }
    let mut moved = bytes.to_vec();
    moved[2] = to;
    let body = moved.len() - 2;
    let sum = checksum(&moved[..body]);
    moved[body..].copy_from_slice(&sum.to_le_bytes());
    Some(moved)
}

/// Reads a change file name like `1793997539-profile-5-keys_easyzone.bin`
/// into its time, profile (counted from 0) and report.
pub fn parse_change_name(name: &str) -> Option<(u64, u8, Report)> {
    let rest = name.strip_suffix(".bin")?;
    let (time, rest) = rest.split_once("-profile-")?;
    let (profile, report_name) = rest.split_once('-')?;
    let profile = profile_index(profile)?;
    let report = *PROFILE_REPORTS.iter().find(|r| r.name == report_name)?;
    Some((time.parse().ok()?, profile, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::test_data::*;

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
    fn valid_profile_report_checks_every_field() {
        assert!(valid_profile_report(&KEYS_THUMBSTER, 0, &THUMBSTER));
        assert!(!valid_profile_report(&KEYS_THUMBSTER, 4, &THUMBSTER));
        assert!(!valid_profile_report(&KEYS_MACRO, 0, &THUMBSTER));
        let mut changed = THUMBSTER;
        changed[3] = 0x1e;
        assert!(!valid_profile_report(&KEYS_THUMBSTER, 0, &changed));
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
    fn for_profile_moves_a_report_and_keeps_it_valid() {
        let moved = for_profile(&KEYS_THUMBSTER, 0, &THUMBSTER, 4).unwrap();
        assert!(valid_profile_report(&KEYS_THUMBSTER, 4, &moved));
        assert_eq!(moved[3..moved.len() - 2], THUMBSTER[3..THUMBSTER.len() - 2]);
        let back = for_profile(&KEYS_THUMBSTER, 4, &moved, 0).unwrap();
        assert_eq!(back, THUMBSTER);
        // Wrong source profile, wrong report, target out of range.
        assert_eq!(for_profile(&KEYS_THUMBSTER, 1, &THUMBSTER, 4), None);
        assert_eq!(for_profile(&KEYS_CAPSLOCK, 0, &THUMBSTER, 4), None);
        assert_eq!(for_profile(&KEYS_THUMBSTER, 0, &THUMBSTER, 5), None);
    }

    #[test]
    fn parse_change_name_reads_real_names() {
        let (time, profile, report) =
            parse_change_name("1793997539-profile-5-keys_easyzone.bin").unwrap();
        assert_eq!(
            (time, profile, report.id),
            (1793997539, 4, KEYS_EASYZONE.id)
        );
        assert_eq!(
            parse_change_name("1-profile-2-light.bin").unwrap().2.id,
            LIGHT.id
        );
        for bad in [
            "1793997539-profile-5-keys_easyzone.bin.undone",
            "1793997539-profile-6-light.bin",
            "1793997539-profile-5-firmware.bin",
            "x-profile-5-light.bin",
            "notes.txt",
        ] {
            assert!(parse_change_name(bad).is_none(), "{bad}");
        }
    }
}

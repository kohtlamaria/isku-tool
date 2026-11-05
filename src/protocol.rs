//! The Isku report format. No USB code here, so all of this can be unit tested.

pub const VENDOR_ID: u16 = 0x1E7D;
pub const PRODUCT_ID: u16 = 0x319C;
/// The settings live on this interface. See docs/protocol.md.
pub const SETTINGS_USAGE_PAGE: u16 = 0x000B;
/// Windows reports this as the feature report length for the settings interface.
pub const FEATURE_BUFFER_LEN: usize = 2083;

#[derive(Clone, Copy, Debug)]
pub struct Report {
    pub id: u8,
    pub len: usize,
    pub name: &'static str,
    /// How many keys the report holds, for key reports.
    pub keys: Option<usize>,
}

/// Reports that are safe to read. `len` is what we saw from the keyboard.
pub const READ_REPORTS: &[Report] = &[
    Report { id: 0x04, len: 3, name: "control", keys: None },
    Report { id: 0x05, len: 3, name: "actual_profile", keys: None },
    Report { id: 0x07, len: 6, name: "key_mask", keys: None },
    Report { id: 0x08, len: 41, name: "keys_function", keys: Some(12) },
    Report { id: 0x09, len: 65, name: "keys_easyzone", keys: Some(20) },
    Report { id: 0x0a, len: 29, name: "keys_media", keys: Some(8) },
    Report { id: 0x0b, len: 23, name: "keys_thumbster", keys: Some(6) },
    Report { id: 0x0d, len: 35, name: "keys_macro", keys: Some(10) },
    Report { id: 0x0f, len: 6, name: "info", keys: None },
    Report { id: 0x10, len: 10, name: "light", keys: None },
    Report { id: 0x13, len: 6, name: "keys_capslock", keys: Some(1) },
    Report { id: 0x14, len: 20, name: "last_set", keys: None },
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

/// The one action byte per key that follows the 3-byte header.
pub fn action_bytes(report: &[u8], keys: usize) -> Option<&[u8]> {
    report.get(3..3 + keys)
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
    fn action_bytes_skip_the_header() {
        assert_eq!(
            action_bytes(&THUMBSTER, 6),
            Some(&[0xbe, 0xbe, 0xbe, 0xb9, 0xb9, 0xb9][..])
        );
        assert_eq!(action_bytes(&CAPSLOCK, 10), None);
    }

    #[test]
    fn report_table_lengths_match_real_reports() {
        let find = |id| READ_REPORTS.iter().find(|r| r.id == id).unwrap();
        assert_eq!(find(0x0b).len, THUMBSTER.len());
        assert_eq!(find(0x13).len, CAPSLOCK.len());
    }
}

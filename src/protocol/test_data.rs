//! Real report bytes shared by the tests.

use super::*;

// Real bytes from the first probe (dumps/2026-11-05-first-probe.txt, kept
// locally and out of git, like all of dumps/).
pub(crate) const THUMBSTER: [u8; 23] = [
    0x0b, 0x17, 0x00, 0xbe, 0xbe, 0xbe, 0xb9, 0xb9, 0xb9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x87,
    0x04,
];

pub(crate) const CAPSLOCK: [u8; 6] = [0x13, 0x06, 0x00, 0x8c, 0xa5, 0x00];

// Profile 1 Easy-Shift zone, unchanged keys, from the same dump.
pub(crate) fn easyzone() -> Vec<u8> {
    let mut report = vec![0x09, 0x41, 0x00];
    report.extend_from_slice(&EASYZONE_DEFAULTS);
    report.extend_from_slice(&[0; 40]);
    report.extend_from_slice(&[0xdc, 0x01]);
    report
}

pub(crate) fn sample_macro(name: &str, strokes: &[(u8, u8)]) -> Vec<u8> {
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

// Profile 1 light report from the same first probe.
pub(crate) const LIGHT_REPORT: [u8; 10] =
    [0x10, 0x0a, 0x00, 0x03, 0x00, 0x05, 0x01, 0x0f, 0x32, 0x00];

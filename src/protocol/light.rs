//! Lighting: brightness, dimming and timeout.

use super::*;

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
pub const LIGHT_DEFAULT: Light = Light {
    brightness: 3,
    dimness: 1,
    timeout: 15,
};

impl Light {
    pub fn is_valid(&self) -> bool {
        self.brightness <= LIGHT_MAX && self.dimness <= LIGHT_MAX && self.timeout >= 1
    }
}

pub fn parse_light(report: &[u8]) -> Option<Light> {
    if report.len() != LIGHT.len || report[0] != LIGHT.id || !checksum_ok(report) {
        return None;
    }
    Some(Light {
        brightness: report[5],
        dimness: report[6],
        timeout: report[7],
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::test_data::*;

    #[test]
    fn parse_light_reads_the_real_report() {
        let light = parse_light(&LIGHT_REPORT).unwrap();
        assert_eq!(
            light,
            Light {
                brightness: 5,
                dimness: 1,
                timeout: 15
            }
        );
        assert_eq!(
            describe_light(light),
            "brightness 5 of 5, dims to 1 after 15 min"
        );
        assert!(valid_profile_report(&LIGHT, 0, &LIGHT_REPORT));
    }

    #[test]
    fn with_light_changes_values_and_keeps_the_report_valid() {
        let updated = with_light(
            &LIGHT_REPORT,
            Light {
                brightness: 2,
                ..LIGHT_DEFAULT
            },
        )
        .unwrap();
        assert!(valid_profile_report(&LIGHT, 0, &updated));
        assert_eq!(parse_light(&updated).unwrap().brightness, 2);
        assert_eq!(updated[3..5], LIGHT_REPORT[3..5]);
        let back = with_light(
            &updated,
            Light {
                brightness: 5,
                dimness: 1,
                timeout: 15,
            },
        )
        .unwrap();
        assert_eq!(back, LIGHT_REPORT);
    }

    #[test]
    fn with_light_refuses_out_of_range_values() {
        for light in [
            Light {
                brightness: 6,
                ..LIGHT_DEFAULT
            },
            Light {
                dimness: 6,
                ..LIGHT_DEFAULT
            },
            Light {
                timeout: 0,
                ..LIGHT_DEFAULT
            },
        ] {
            assert_eq!(with_light(&LIGHT_REPORT, light), None);
        }
        assert_eq!(with_light(&CAPSLOCK, LIGHT_DEFAULT), None);
    }
}

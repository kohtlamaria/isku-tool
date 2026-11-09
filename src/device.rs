//! Talks to the keyboard. Four things can be sent: the select command, the
//! active profile, per-profile key and light reports, and macros. Reports
//! and macros must be valid for their profile and are read back after
//! writing.

use crate::protocol::{
    actual_profile_report, checksum_ok, control_report, valid_macro_report, valid_profile_report,
    Report, ACTUAL_PROFILE, CONTROL_REPORT_ID, FEATURE_BUFFER_LEN, MACRO_REPORT_ID, PRODUCT_ID,
    PROFILE_COUNT, PROFILE_REPORTS, SETTINGS_USAGE_PAGE, STATUS_BUSY, STATUS_EMPTY, STATUS_OK,
    VENDOR_ID,
};
use hidapi::{HidApi, HidDevice};
use std::cell::Cell;
use std::thread::sleep;
use std::time::{Duration, Instant};

/// Writes wait as long as roccat-tools does before the first status check.
const WRITE_WAIT: Duration = Duration::from_millis(110);
/// Selects only choose what to read, so they are checked almost at once. If the
/// keyboard has not caught up, the read that follows is checked and repeated.
const SELECT_WAIT: Duration = Duration::from_millis(5);
const POLL_WAIT: Duration = Duration::from_millis(10);
const STATUS_TIMEOUT: Duration = Duration::from_secs(3);
const READ_ATTEMPTS: u32 = 20;

/// Counts where time goes, so slow runs can be explained.
#[derive(Default)]
struct Stats {
    reads: Cell<u32>,
    read_time: Cell<Duration>,
    commands: Cell<u32>,
    busy: Cell<u32>,
    retries: Cell<u32>,
    wait_time: Cell<Duration>,
}

fn add(cell: &Cell<Duration>, start: Instant) {
    cell.set(cell.get() + start.elapsed());
}

pub struct Isku {
    device: HidDevice,
    stats: Stats,
}

impl Isku {
    pub fn open() -> Result<Self, String> {
        let api = HidApi::new().map_err(|e| e.to_string())?;
        let info = api
            .device_list()
            .find(|d| {
                d.vendor_id() == VENDOR_ID
                    && d.product_id() == PRODUCT_ID
                    && d.usage_page() == SETTINGS_USAGE_PAGE
            })
            .ok_or("Isku settings interface not found")?;
        let device = info.open_device(&api).map_err(|e| e.to_string())?;
        Ok(Self {
            device,
            stats: Stats::default(),
        })
    }

    /// One line on what the keyboard spent time on since it was opened.
    pub fn timing(&self) -> String {
        let s = &self.stats;
        format!(
            "{} reads took {:.1} s, {} commands waited {:.1} s, busy {} times, reread {} times",
            s.reads.get(),
            s.read_time.get().as_secs_f64(),
            s.commands.get(),
            s.wait_time.get().as_secs_f64(),
            s.busy.get(),
            s.retries.get()
        )
    }

    fn read_raw(&self, id: u8, len: usize) -> Result<Vec<u8>, String> {
        let start = Instant::now();
        let mut buffer = vec![0u8; FEATURE_BUFFER_LEN];
        buffer[0] = id;
        let result = self
            .device
            .get_feature_report(&mut buffer)
            .map_err(|e| format!("reading report 0x{id:02x}: {e}"));
        self.stats.reads.set(self.stats.reads.get() + 1);
        add(&self.stats.read_time, start);
        // Catches short reads where hidapi reports the real size. On Windows it
        // reports the padded buffer size, so callers also check each report's
        // own length byte or checksum.
        let got = result?;
        if got < len {
            return Err(format!(
                "report 0x{id:02x}: keyboard sent {got} bytes, expected {len}"
            ));
        }
        buffer.truncate(len);
        Ok(buffer)
    }

    fn send_and_wait(&self, bytes: &[u8], first_wait: Duration) -> Result<(), String> {
        ok_status(self.send_and_status(bytes, first_wait)?)
    }

    /// Sends a command and waits until the keyboard is no longer busy, then
    /// returns its status.
    fn send_and_status(&self, bytes: &[u8], first_wait: Duration) -> Result<u8, String> {
        self.device
            .send_feature_report(bytes)
            .map_err(|e| format!("sending report 0x{:02x}: {e}", bytes[0]))?;
        self.stats.commands.set(self.stats.commands.get() + 1);
        let started = Instant::now();
        let mut wait = first_wait;
        while started.elapsed() < STATUS_TIMEOUT {
            let start = Instant::now();
            sleep(wait);
            add(&self.stats.wait_time, start);
            match self.read_raw(CONTROL_REPORT_ID, 3)?[1] {
                STATUS_BUSY => self.stats.busy.set(self.stats.busy.get() + 1),
                status => return Ok(status),
            }
            wait = POLL_WAIT;
        }
        Err("keyboard stayed busy".to_string())
    }

    /// Reads a report until `accept` passes, in case the keyboard answered
    /// before it had switched to the selected data.
    fn read_until(
        &self,
        id: u8,
        len: usize,
        accept: impl Fn(&[u8]) -> Result<(), String>,
    ) -> Result<Vec<u8>, String> {
        let mut last_error = String::new();
        for attempt in 0..READ_ATTEMPTS {
            if attempt > 0 {
                self.stats.retries.set(self.stats.retries.get() + 1);
                sleep(POLL_WAIT);
            }
            let bytes = self.read_raw(id, len)?;
            match accept(&bytes) {
                Ok(()) => return Ok(bytes),
                Err(error) => last_error = error,
            }
        }
        Err(last_error)
    }

    fn select(&self, profile: u8, request: u8) -> Result<(), String> {
        ok_status(self.select_status(profile, request)?)
    }

    /// Selects a report or macro and returns the keyboard's status.
    fn select_status(&self, profile: u8, request: u8) -> Result<u8, String> {
        let command = control_report(profile, request).ok_or_else(|| {
            format!("refusing select: profile {profile}, request 0x{request:02x}")
        })?;
        self.send_and_status(&command, SELECT_WAIT)
    }

    /// The active profile, counted from 0.
    pub fn active_profile(&self) -> Result<u8, String> {
        let bytes = self.read_raw(ACTUAL_PROFILE.id, ACTUAL_PROFILE.len)?;
        // This report has no checksum, so check its id, length byte and value.
        if bytes[0] != ACTUAL_PROFILE.id
            || usize::from(bytes[1]) != ACTUAL_PROFILE.len
            || bytes[2] >= PROFILE_COUNT
        {
            return Err(format!("bad actual_profile report: {bytes:02x?}"));
        }
        Ok(bytes[2])
    }

    pub fn activate_profile(&self, profile: u8) -> Result<(), String> {
        let command = actual_profile_report(profile).ok_or("no such profile")?;
        self.send_and_wait(&command, WRITE_WAIT)?;
        if self.active_profile()? != profile {
            return Err("the keyboard did not switch profile".to_string());
        }
        Ok(())
    }

    pub fn read_global(&self, report: &Report) -> Result<Vec<u8>, String> {
        let bytes = self.read_raw(report.id, report.len)?;
        check(report, &bytes)?;
        Ok(bytes)
    }

    pub fn read_profile_report(&self, profile: u8, report: &Report) -> Result<Vec<u8>, String> {
        let request = report
            .request
            .ok_or_else(|| format!("{} is not per profile", report.name))?;
        self.select(profile, request)?;
        self.read_until(report.id, report.len, |bytes| {
            check(report, bytes)?;
            if bytes[2] != profile {
                return Err(format!(
                    "{}: asked for profile {profile}, got {}",
                    report.name, bytes[2]
                ));
            }
            Ok(())
        })
    }

    /// Writes a report to a profile, then reads it back to confirm.
    pub fn write_profile_report(
        &self,
        profile: u8,
        report: &Report,
        bytes: &[u8],
    ) -> Result<(), String> {
        check_writable(profile, report, bytes)?;
        self.send_and_wait(bytes, WRITE_WAIT)?;
        if self.read_profile_report(profile, report)? != bytes {
            return Err(format!("{}: read back different bytes", report.name));
        }
        Ok(())
    }

    /// Reads one macro. Returns None when the slot holds no macro.
    pub fn read_macro(&self, profile: u8, key_index: u8) -> Result<Option<Vec<u8>>, String> {
        match self.select_status(profile, key_index)? {
            STATUS_EMPTY => return Ok(None),
            status => ok_status(status)?,
        }
        self.read_until(MACRO_REPORT_ID, FEATURE_BUFFER_LEN, |bytes| {
            if !checksum_ok(bytes) {
                return Err(format!("macro 0x{key_index:02x}: bad checksum"));
            }
            if bytes[3] != profile || bytes[4] != key_index {
                return Err(format!(
                    "macro 0x{key_index:02x}: got profile {} key 0x{:02x}",
                    bytes[3], bytes[4]
                ));
            }
            Ok(())
        })
        .map(Some)
    }

    /// Writes one macro, then reads it back to confirm.
    pub fn write_macro(&self, profile: u8, key_index: u8, bytes: &[u8]) -> Result<(), String> {
        check_macro_writable(profile, key_index, bytes)?;
        self.send_and_wait(bytes, WRITE_WAIT)?;
        match self.read_macro(profile, key_index)? {
            Some(back) if back == bytes => Ok(()),
            Some(_) => Err(format!(
                "macro 0x{key_index:02x}: read back different bytes"
            )),
            None => Err(format!(
                "macro 0x{key_index:02x}: the slot is empty after writing"
            )),
        }
    }
}

/// The check every macro write passes before anything is sent: only a
/// complete, valid macro for that profile and slot.
pub fn check_macro_writable(profile: u8, key_index: u8, bytes: &[u8]) -> Result<(), String> {
    if valid_macro_report(profile, key_index, bytes) {
        Ok(())
    } else {
        Err(format!(
            "macro 0x{key_index:02x}: refusing to write an invalid macro"
        ))
    }
}

/// The checks every write passes before anything is sent: only per-profile
/// reports, and only complete, valid ones for that profile.
pub fn check_writable(profile: u8, report: &Report, bytes: &[u8]) -> Result<(), String> {
    if !PROFILE_REPORTS.iter().any(|r| r.id == report.id) {
        return Err(format!("{} cannot be written", report.name));
    }
    if !valid_profile_report(report, profile, bytes) {
        return Err(format!(
            "{}: refusing to write an invalid report",
            report.name
        ));
    }
    Ok(())
}

fn ok_status(status: u8) -> Result<(), String> {
    match status {
        STATUS_OK => Ok(()),
        status => Err(format!("keyboard answered status {status}")),
    }
}

fn check(report: &Report, bytes: &[u8]) -> Result<(), String> {
    if report.has_checksum && !checksum_ok(bytes) {
        return Err(format!("{}: bad checksum", report.name));
    }
    Ok(())
}
